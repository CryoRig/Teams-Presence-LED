use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Instant, Duration};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};

const INITIAL_READ_TAIL_BYTES: u64 = 256 * 1024;
const TEAMS_PROCESS_NAME: &str = "ms-teams.exe";
const PROCESS_CHECK_INTERVAL: Duration = Duration::from_secs(5);
/// Presence reported while the Teams process is not running (maps to the "Offline" entry).
const PRESENCE_OFFLINE: &str = "Offline";

pub struct TeamsClient {
    log_directory: PathBuf,
    current_file: Option<PathBuf>,
    last_position: u64,
    last_presence: Option<String>,
    last_dir_scan: Option<Instant>,
    system: System,
    last_process_check: Option<Instant>,
    teams_running: bool,
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
            
        eprintln!("[TeamsClient] Initialized using log directory: {:?}", log_directory);

        Self {
            log_directory,
            current_file: None,
            last_position: 0,
            last_presence: None,
            last_dir_scan: None,
            system: System::new(),
            last_process_check: None,
            teams_running: false,
        }
    }

    pub fn has_valid_log(&self) -> bool {
        self.current_file
            .as_ref()
            .is_some_and(|path| path.exists())
    }

    fn is_teams_running(&mut self) -> bool {
        if self.last_process_check.is_none_or(|t| t.elapsed() >= PROCESS_CHECK_INTERVAL) {
            self.system.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
            let running = self
                .system
                .processes()
                .values()
                .any(|p| p.name().eq_ignore_ascii_case(TEAMS_PROCESS_NAME));
            if running != self.teams_running {
                eprintln!("[TeamsClient] {} is {}", TEAMS_PROCESS_NAME, if running { "running" } else { "not running" });
            }
            self.teams_running = running;
            self.last_process_check = Some(Instant::now());
        }
        self.teams_running
    }

    pub fn get_presence(&mut self) -> Option<String> {
        if !self.is_teams_running() {
            // Drop the stale state so a restarted Teams does not briefly replay it
            self.last_presence = None;
            return Some(PRESENCE_OFFLINE.to_string());
        }

        if !self.log_directory.exists() {
            return None;
        }

        let mut newest_file = self.current_file.clone();
        
        let needs_scan = self.last_dir_scan.is_none_or(|last| last.elapsed() > Duration::from_secs(60));
        
        if needs_scan {
            let mut max_time = std::time::SystemTime::UNIX_EPOCH;
            if let Ok(entries) = fs::read_dir(&self.log_directory) {
                for entry in entries.filter_map(Result::ok) {
                    let path = entry.path();
                    if path.is_file()
                        && let Some(name) = path.file_name().and_then(|n| n.to_str())
                            && name.starts_with("MSTeams_") && name.ends_with(".log")
                                && let Ok(metadata) = entry.metadata()
                                    && let Ok(time) = metadata.modified()
                                        && time > max_time {
                                            max_time = time;
                                            newest_file = Some(path);
                                        }
                }
            }
            self.last_dir_scan = Some(Instant::now());
        }

        if let Some(file_path) = newest_file {
            if self.current_file != Some(file_path.clone()) {
                if self.current_file.is_some() {
                    eprintln!("[TeamsClient] Log rotated -> {:?}", file_path.file_name().unwrap());
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
                    if self.last_position == 0 {
                        self.last_position = metadata.len().saturating_sub(INITIAL_READ_TAIL_BYTES);
                    }
                }

                if file.seek(SeekFrom::Start(self.last_position)).is_ok() {
                    let mut contents = Vec::new();
                    if file.read_to_end(&mut contents).is_ok() {
                        let last_nl = contents.iter().rposition(|&c| c == b'\n');
                        if let Some(nl_pos) = last_nl {
                            self.last_position += (nl_pos + 1) as u64;
                            let valid_bytes = &contents[..=nl_pos];
                            let text = String::from_utf8_lossy(valid_bytes);

                            let mut current_status = None;
                            let prefix = "UserPresenceAction:";
                            let avail_key = "availability: ";
                            for line in text.lines() {
                                if let Some(start) = line.find(prefix) {
                                    let remainder = &line[start + prefix.len()..];
                                    if let Some(avail_start) = remainder.find(avail_key) {
                                        let status_start = avail_start + avail_key.len();
                                        let status_remainder = &remainder[status_start..];
                                        // The status goes up to the closing brace '}'
                                        if let Some(end) = status_remainder.find('}') {
                                            let status = status_remainder[..end].trim();
                                            if status != "PresenceUnknown" {
                                                current_status = Some(status.to_string());
                                            }
                                        }
                                    }
                                }
                            }

                            if let Some(status) = current_status
                                && Some(&status) != self.last_presence.as_ref() {
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
