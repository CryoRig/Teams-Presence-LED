use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Instant, Duration};

pub struct TeamsClient {
    log_directory: PathBuf,
    current_file: Option<PathBuf>,
    last_position: u64,
    last_presence: Option<String>,
    last_dir_scan: Option<Instant>,
}

impl TeamsClient {
    pub fn new() -> Self {
        let local_app_data = std::env::var("LOCALAPPDATA").unwrap_or_else(|e| {
            eprintln!("[TeamsClient] LOCALAPPDATA not set: {}. Teams log parsing will be disabled.", e);
            String::new()
        });
        let log_directory = Path::new(&local_app_data)
            .join("Packages")
            .join("MSTeams_8wekyb3d8bbwe")
            .join("LocalCache")
            .join("Microsoft")
            .join("MSTeams")
            .join("Logs");
            
        println!("[TeamsClient] Initialized using log directory: {:?}", log_directory);

        Self {
            log_directory,
            current_file: None,
            last_position: 0,
            last_presence: None,
            last_dir_scan: None,
        }
    }

    pub fn has_valid_log(&self) -> bool {
        self.current_file.is_some()
    }

    pub fn get_presence(&mut self) -> Option<String> {
        if !self.log_directory.exists() {
            return None;
        }

        let mut newest_file = self.current_file.clone();
        
        let needs_scan = self.last_dir_scan.map_or(true, |last| last.elapsed() > Duration::from_secs(60));
        
        if needs_scan {
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
            self.last_dir_scan = Some(Instant::now());
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
                        let prefix = "UserPresenceAction: {cloud_context: https://teams.microsoft.com, availability: ";
                        for line in contents.lines() {
                            if let Some(start) = line.find(prefix) {
                                let remainder = &line[start + prefix.len()..];
                                if let Some(end) = remainder.find('}') {
                                    let status = &remainder[..end];
                                    if status != "PresenceUnknown" {
                                        current_status = Some(status.to_string());
                                    }
                                }
                            }
                        }

                        if let Some(status) = current_status {
                            if Some(&status) != self.last_presence.as_ref() {
                                self.last_presence = Some(status);
                            }
                        }
                    }
                }
            }
        }

        self.last_presence.clone()
    }
}
