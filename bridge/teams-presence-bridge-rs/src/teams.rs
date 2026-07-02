use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use regex::Regex;

pub struct TeamsClient {
    log_directory: PathBuf,
    current_file: Option<PathBuf>,
    last_position: u64,
    last_presence: Arc<Mutex<Option<String>>>,
    presence_regex: Regex,
}

impl TeamsClient {
    pub fn new() -> Self {
        let local_app_data = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| String::new());
        let log_directory = Path::new(&local_app_data)
            .join("Packages")
            .join("MSTeams_8wekyb3d8bbwe")
            .join("LocalCache")
            .join("Microsoft")
            .join("MSTeams")
            .join("Logs");
            
        println!("[TeamsClient] Initialized using log directory: {:?}", log_directory);

        let last_presence = Arc::new(Mutex::new(None));
        let presence_regex = Regex::new(r"UserPresenceAction: \{cloud_context: https://teams\.microsoft\.com, availability: (\w+)\}").unwrap();

        Self {
            log_directory,
            current_file: None,
            last_position: 0,
            last_presence,
            presence_regex,
        }
    }

    pub fn has_valid_log(&self) -> bool {
        self.current_file.is_some()
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
                    println!("[TeamsClient] Log rotated -> {:?}", file_path.file_name().unwrap());
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

                        let mut current_status = None;
                        for line in contents.lines() {
                            if line.contains("UserPresenceAction") {
                                if let Some(cap) = self.presence_regex.captures(line) {
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
