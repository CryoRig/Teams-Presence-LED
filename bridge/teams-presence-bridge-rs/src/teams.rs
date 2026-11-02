use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Instant, Duration};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};

const INITIAL_READ_TAIL_BYTES: u64 = 256 * 1024;
const TEAMS_PROCESS_NAME: &str = "ms-teams.exe";
const PROCESS_CHECK_INTERVAL: Duration = Duration::from_secs(5);
const DIR_SCAN_INTERVAL: Duration = Duration::from_secs(60);
/// A log that stops growing for this long triggers an early rotation check.
const QUIET_LOG_RESCAN_INTERVAL: Duration = Duration::from_secs(15);
/// Presence reported while the Teams process is not running (maps to the "Offline" entry).
const PRESENCE_OFFLINE: &str = "Offline";

pub struct TeamsClient {
    log_directory: PathBuf,
    current_file: Option<PathBuf>,
    last_position: u64,
    last_file_len: u64,
    last_growth: Option<Instant>,
    last_presence: Option<String>,
    last_dir_scan: Option<Instant>,
    system: Option<System>,
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

        let system = System::new().map_err(|error| {
            eprintln!("[TeamsClient] Could not initialize process monitor: {error}");
            error
        }).ok();

        let mut client = Self::with_log_directory(log_directory);
        client.system = system;
        client
    }

    /// Client without a process monitor; presence comes purely from the given log directory.
    fn with_log_directory(log_directory: PathBuf) -> Self {
        Self {
            log_directory,
            current_file: None,
            last_position: 0,
            last_file_len: 0,
            last_growth: None,
            last_presence: None,
            last_dir_scan: None,
            system: None,
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
        // Without a process monitor, assume Teams is running so log parsing still works.
        let Some(system) = self.system.as_mut() else {
            return true;
        };
        if self.last_process_check.is_none_or(|t| t.elapsed() >= PROCESS_CHECK_INTERVAL) {
            system.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
            let running = system.processes().values().any(|p| p.name().eq_ignore_ascii_case(TEAMS_PROCESS_NAME));
            if running != self.teams_running {
                eprintln!("[TeamsClient] {} is {}", TEAMS_PROCESS_NAME, if running { "running" } else { "not running" });
            }
            self.teams_running = running;
            self.last_process_check = Some(Instant::now());
        }
        self.teams_running
    }

    /// Rescan the log directory when the current file vanished, the periodic interval elapsed,
    /// or the current file has been quiet long enough that Teams may have rotated to a new one.
    fn needs_dir_scan(&self, now: Instant) -> bool {
        let Some(last_scan) = self.last_dir_scan else {
            return true;
        };
        let since_scan = now.duration_since(last_scan);
        if since_scan > DIR_SCAN_INTERVAL {
            return true;
        }
        if !self.has_valid_log() {
            return true;
        }
        since_scan >= QUIET_LOG_RESCAN_INTERVAL
            && self.last_growth.is_some_and(|t| now.duration_since(t) >= QUIET_LOG_RESCAN_INTERVAL)
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

        let now = Instant::now();
        let mut newest_file = self.current_file.clone();

        if self.needs_dir_scan(now) {
            if let Some(path) = newest_log_file(&self.log_directory) {
                newest_file = Some(path);
            }
            self.last_dir_scan = Some(now);
        }

        if let Some(file_path) = newest_file {
            if self.current_file.as_ref() != Some(&file_path) {
                if self.current_file.is_some() {
                    eprintln!("[TeamsClient] Log rotated -> {}", file_path.display());
                }
                self.current_file = Some(file_path.clone());
                self.last_position = 0;
                self.last_file_len = 0;
                self.last_growth = Some(now);
            }

            if let Ok(mut file) = File::open(&file_path) {
                if let Ok(metadata) = file.metadata() {
                    let len = metadata.len();
                    // Shrunk in place: treat as a fresh file.
                    if len < self.last_position {
                        self.last_position = 0;
                    }
                    if self.last_position == 0 {
                        self.last_position = len.saturating_sub(INITIAL_READ_TAIL_BYTES);
                    }
                    if len != self.last_file_len {
                        self.last_file_len = len;
                        self.last_growth = Some(now);
                    }
                }

                if file.seek(SeekFrom::Start(self.last_position)).is_ok() {
                    let mut contents = Vec::new();
                    if file.read_to_end(&mut contents).is_ok()
                        && let Some(nl_pos) = contents.iter().rposition(|&c| c == b'\n')
                    {
                        // Only consume complete lines; a partial tail is re-read next poll.
                        self.last_position += (nl_pos + 1) as u64;
                        let text = String::from_utf8_lossy(&contents[..=nl_pos]);
                        if let Some(status) = extract_presence(&text) {
                            self.last_presence = Some(status);
                        }
                    }
                }
            }
        }

        self.last_presence.clone()
    }
}

/// Newest `MSTeams_*.log` by modification time, or `None` if the directory has none.
fn newest_log_file(dir: &Path) -> Option<PathBuf> {
    let mut newest: Option<(std::time::SystemTime, PathBuf)> = None;
    for entry in fs::read_dir(dir).ok()?.filter_map(Result::ok) {
        let path = entry.path();
        let is_log = path.is_file()
            && path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|name| name.starts_with("MSTeams_") && name.ends_with(".log"));
        if !is_log {
            continue;
        }
        if let Ok(time) = entry.metadata().and_then(|m| m.modified())
            && newest.as_ref().is_none_or(|(t, _)| time > *t)
        {
            newest = Some((time, path));
        }
    }
    newest.map(|(_, path)| path)
}

/// Last availability value from `UserPresenceAction` lines, ignoring `PresenceUnknown`.
fn extract_presence(text: &str) -> Option<String> {
    const PREFIX: &str = "UserPresenceAction:";
    const AVAIL_KEY: &str = "availability: ";

    let mut current_status = None;
    for line in text.lines() {
        let Some(start) = line.find(PREFIX) else { continue };
        let remainder = &line[start + PREFIX.len()..];
        let Some(avail_start) = remainder.find(AVAIL_KEY) else { continue };
        let status_remainder = &remainder[avail_start + AVAIL_KEY.len()..];
        // The status runs up to the closing brace of the action object.
        if let Some(end) = status_remainder.find('}') {
            let status = status_remainder[..end].trim();
            if !status.is_empty() && status != "PresenceUnknown" {
                current_status = Some(status.to_string());
            }
        }
    }
    current_status
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn client_for(dir: &Path) -> TeamsClient {
        TeamsClient::with_log_directory(dir.to_path_buf())
    }

    fn write_log(path: &Path, lines: &[&str]) {
        let mut f = fs::OpenOptions::new().create(true).append(true).open(path).unwrap();
        for line in lines {
            writeln!(f, "{line}").unwrap();
        }
    }

    fn presence_line(status: &str) -> String {
        format!("2026-11-02T10:00:00 Inf UserPresenceAction: {{availability: {status}}}")
    }

    #[test]
    fn extract_presence_returns_last_known_status() {
        let text = format!("{}\n{}\nnoise\n", presence_line("Busy"), presence_line("Available"));
        assert_eq!(extract_presence(&text).as_deref(), Some("Available"));
    }

    #[test]
    fn extract_presence_skips_presence_unknown_and_garbage() {
        let text = format!("{}\n{}\nUserPresenceAction: no key here\n", presence_line("Away"), presence_line("PresenceUnknown"));
        assert_eq!(extract_presence(&text).as_deref(), Some("Away"));
        assert_eq!(extract_presence("nothing relevant\n"), None);
    }

    #[test]
    fn partial_trailing_line_is_reprocessed_on_next_poll() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("MSTeams_2026-11-02_10-00-00.log");
        write_log(&log, &[&presence_line("Busy")]);
        // Partial line without newline.
        fs::OpenOptions::new().append(true).open(&log).unwrap()
            .write_all(presence_line("Available").split_at(20).0.as_bytes()).unwrap();

        let mut client = client_for(dir.path());
        assert_eq!(client.get_presence().as_deref(), Some("Busy"));

        fs::OpenOptions::new().append(true).open(&log).unwrap()
            .write_all(format!("{}\n", presence_line("Available").split_at(20).1).as_bytes()).unwrap();
        assert_eq!(client.get_presence().as_deref(), Some("Available"));
    }

    #[test]
    fn rotation_to_newer_log_is_picked_up_when_old_log_is_removed() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("MSTeams_old.log");
        write_log(&old, &[&presence_line("Busy")]);
        let mut client = client_for(dir.path());
        assert_eq!(client.get_presence().as_deref(), Some("Busy"));
        assert!(client.has_valid_log());

        fs::remove_file(&old).unwrap();
        let new = dir.path().join("MSTeams_new.log");
        write_log(&new, &[&presence_line("DoNotDisturb")]);
        // The missing current file forces a rescan without waiting for the periodic interval.
        assert_eq!(client.get_presence().as_deref(), Some("DoNotDisturb"));
        assert_eq!(client.current_file.as_deref(), Some(new.as_path()));
    }

    #[test]
    fn newest_log_file_ignores_non_matching_names() {
        let dir = tempfile::tempdir().unwrap();
        write_log(&dir.path().join("other.log"), &["x"]);
        assert!(newest_log_file(dir.path()).is_none());
        write_log(&dir.path().join("MSTeams_a.log"), &["x"]);
        assert!(newest_log_file(dir.path()).is_some());
    }
}
