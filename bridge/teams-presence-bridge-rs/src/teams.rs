use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use notify::{Watcher, RecursiveMode, Event, EventKind};
use notify::event::AccessKind;
use regex::Regex;
use reqwest::blocking::Client;
use serde_json::Value;

pub struct TeamsClient {
    http: Client,
    active_port: Option<u16>,
    is_fallback_active: bool,
    log_watcher: Option<TeamsLogWatcher>,
}

impl TeamsClient {
    const KNOWN_PORTS: [u16; 2] = [8124, 8125];

    pub fn new() -> Self {
        Self {
            http: Client::builder()
                .timeout(Duration::from_secs(3))
                .build()
                .unwrap(),
            active_port: None,
            is_fallback_active: false,
            log_watcher: None,
        }
    }

    pub fn get_presence(&mut self) -> Option<String> {
        if self.is_fallback_active {
            if let Some(watcher) = &mut self.log_watcher {
                return watcher.get_presence();
            }
            return None;
        }

        if let Some(port) = self.active_port {
            if let Some(presence) = self.try_get_presence_from_port(port) {
                return Some(presence);
            }
            self.active_port = None;
        }

        for port in Self::KNOWN_PORTS {
            if let Some(presence) = self.try_get_presence_from_port(port) {
                self.active_port = Some(port);
                println!("[TeamsClient] Found Teams API on port {}", port);
                return Some(presence);
            }
        }

        println!("[TeamsClient] Local HTTP API not responding. Falling back to logfile parsing.");
        self.is_fallback_active = true;
        
        let mut watcher = TeamsLogWatcher::new();
        let presence = watcher.get_presence();
        self.log_watcher = Some(watcher);
        
        presence
    }

    fn try_get_presence_from_port(&self, port: u16) -> Option<String> {
        let url = format!("http://localhost:{}/presenceState", port);
        if let Ok(resp) = self.http.get(&url).send() {
            if let Ok(json) = resp.json::<Value>() {
                if let Some(state) = json.get("availability")
                    .or_else(|| json.get("state"))
                    .or_else(|| json.get("presenceState"))
                {
                    if let Some(s) = state.as_str() {
                        return Some(s.to_string());
                    }
                }
            }
        }
        None
    }
}

pub struct TeamsLogWatcher {
    log_directory: PathBuf,
    current_file: Option<PathBuf>,
    last_position: u64,
    last_presence: Arc<Mutex<Option<String>>>,
    _watcher: Option<notify::RecommendedWatcher>,
}

impl TeamsLogWatcher {
    pub fn new() -> Self {
        let local_app_data = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| String::new());
        let log_directory = Path::new(&local_app_data)
            .join("Packages")
            .join("MSTeams_8wekyb3d8bbwe")
            .join("LocalCache")
            .join("Microsoft")
            .join("MSTeams")
            .join("Logs");
            
        println!("[TeamsLogWatcher] Initialized using log directory: {:?}", log_directory);

        let last_presence = Arc::new(Mutex::new(None));
        
        // Background watcher thread for notify
        let mut watcher = None;
        
        if log_directory.exists() {
            let _log_dir_clone = log_directory.clone();
            if let Ok(mut w) = notify::recommended_watcher(move |res: notify::Result<Event>| {
                match res {
                    Ok(event) => {
                        // Check if it's a write event
                        if let EventKind::Access(AccessKind::Close(_)) | EventKind::Modify(_) = event.kind {
                            // We just set a flag or we could parse inline. 
                            // Because notify is async to our main loop, we'll just let get_presence pull the latest file changes
                            // Actually, let's keep it simple: get_presence() reads the delta when called.
                            // notify here is just to reduce CPU load if we wanted an event-driven architecture.
                            // Since we have a main loop for ping/reconnect anyway, we can just poll the file size delta in get_presence().
                        }
                    }
                    Err(_) => {}
                }
            }) {
                let _ = w.watch(&log_directory, RecursiveMode::NonRecursive);
                watcher = Some(w);
            }
        }

        Self {
            log_directory,
            current_file: None,
            last_position: 0,
            last_presence,
            _watcher: watcher,
        }
    }

    pub fn get_presence(&mut self) -> Option<String> {
        if !self.log_directory.exists() {
            return None;
        }

        let mut newest_file = None;
        let mut max_time = std::time::SystemTime::UNIX_EPOCH;

        if let Ok(entries) = fs::read_dir(&self.log_directory) {
            for entry in entries.filter_map(Result::ok) {
                let path = entry.path();
                if path.is_file() {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        if name.starts_with("MSTeams_") && name.ends_with(".log") {
                            if let Ok(metadata) = entry.metadata() {
                                if let Ok(time) = metadata.modified() {
                                    if time > max_time {
                                        max_time = time;
                                        newest_file = Some(path);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if let Some(file_path) = newest_file {
            if self.current_file != Some(file_path.clone()) {
                if self.current_file.is_some() {
                    println!("[TeamsLogWatcher] Log rotated -> {:?}", file_path.file_name().unwrap());
                }
                self.current_file = Some(file_path.clone());
                self.last_position = 0;
            }

            if let Ok(mut file) = File::open(&file_path) {
                // Check if file shrank (rotated but same name?)
                if let Ok(metadata) = file.metadata() {
                    if metadata.len() < self.last_position {
                        self.last_position = 0;
                    }
                }

                if let Ok(_) = file.seek(SeekFrom::Start(self.last_position)) {
                    let mut contents = String::new();
                    if let Ok(_) = file.read_to_string(&mut contents) {
                        self.last_position += contents.len() as u64;

                        // Optimized parsing: don't run regex if line doesn't contain UserPresenceAction
                        let re = Regex::new(r"UserPresenceAction: \{cloud_context: https://teams\.microsoft\.com, availability: (\w+)\}").unwrap();
                        
                        let mut current_status = None;
                        for line in contents.lines() {
                            if line.contains("UserPresenceAction") {
                                if let Some(cap) = re.captures(line) {
                                    let status = cap.get(1).unwrap().as_str();
                                    if status != "PresenceUnknown" {
                                        current_status = Some(status.to_string());
                                    }
                                }
                            }
                        }

                        if let Some(status) = current_status {
                            let mut lock = self.last_presence.lock().unwrap();
                            if Some(&status) != lock.as_ref() {
                                *lock = Some(status);
                            }
                        }
                    }
                }
            }
        }

        let lock = self.last_presence.lock().unwrap();
        lock.clone()
    }
}
