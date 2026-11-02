use std::collections::HashMap;
use std::fs;
use std::path::Path;
use serde::{Deserialize, Serialize};

// Firmware watchdog fires after 60 s without a command; keep pings well below that.
pub const MIN_POLL_INTERVAL_MS: u64 = 100;
pub const MAX_POLL_INTERVAL_MS: u64 = 10_000;
pub const MIN_PING_INTERVAL_MS: u64 = 1_000;
pub const MAX_PING_INTERVAL_MS: u64 = 30_000;

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    #[serde(default = "default_poll_interval")]
    pub poll_interval_ms: u64,
    #[serde(default = "default_ping_interval")]
    pub ping_interval_ms: u64,
    #[serde(default = "default_brightness")]
    pub brightness: u8,
    #[serde(default = "default_transition")]
    pub transition_duration_ms: u16,
    #[serde(default)]
    pub presence_map: HashMap<String, ColorCommand>,
}

fn default_poll_interval() -> u64 {
    5000
}

fn default_ping_interval() -> u64 {
    15000
}

fn default_brightness() -> u8 {
    191 // ~75% of 255
}

fn default_transition() -> u16 {
    500
}

impl Config {
    pub fn clamp_ranges(&mut self) {
        self.poll_interval_ms = self.poll_interval_ms.clamp(MIN_POLL_INTERVAL_MS, MAX_POLL_INTERVAL_MS);
        self.ping_interval_ms = self.ping_interval_ms.clamp(MIN_PING_INTERVAL_MS, MAX_PING_INTERVAL_MS);
    }
}

impl Default for Config {
    fn default() -> Self {
        use LedMode::*;
        let mut presence_map = HashMap::new();
        presence_map.insert("DoNotDisturb".into(), ColorCommand { command: Solid, r: 200, g: 0, b: 0 });
        presence_map.insert("Away".into(), ColorCommand { command: BreatheSlow, r: 255, g: 120, b: 0 });
        presence_map.insert("Busy".into(), ColorCommand { command: Solid, r: 200, g: 0, b: 0 });
        presence_map.insert("BusyIdle".into(), ColorCommand { command: BreatheSlow, r: 200, g: 0, b: 0 });
        presence_map.insert("BeRightBack".into(), ColorCommand { command: BreatheSlow, r: 255, g: 80, b: 0 });
        presence_map.insert("Unknown".into(), ColorCommand { command: Breathe, r: 80, g: 80, b: 80 });
        presence_map.insert("Available".into(), ColorCommand { command: Solid, r: 0, g: 200, b: 0 });
        presence_map.insert("AvailableIdle".into(), ColorCommand { command: BreatheSlow, r: 255, g: 120, b: 0 });
        presence_map.insert("Offline".into(), ColorCommand { command: Off, r: 0, g: 0, b: 0 });

        Self {
            poll_interval_ms: default_poll_interval(),
            ping_interval_ms: default_ping_interval(),
            brightness: default_brightness(),
            transition_duration_ms: default_transition(),
            presence_map,
        }
    }
}

/// LED behaviour for a presence state; serialized as the protocol command name.
#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LedMode {
    Off,
    Solid,
    Breathe,
    BreatheSlow,
}

impl LedMode {
    pub const ALL: [LedMode; 4] = [LedMode::Off, LedMode::Solid, LedMode::Breathe, LedMode::BreatheSlow];

    /// HID command ID (protocol.md).
    pub fn command_id(self) -> u8 {
        match self {
            LedMode::Off => 0x02,
            LedMode::Solid => 0x03,
            LedMode::Breathe => 0x04,
            LedMode::BreatheSlow => 0x05,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            LedMode::Off => "OFF",
            LedMode::Solid => "SOLID",
            LedMode::Breathe => "BREATHE",
            LedMode::BreatheSlow => "BREATHE_SLOW",
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ColorCommand {
    pub command: LedMode,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl ColorCommand {
    pub fn to_hid_params(&self) -> (u8, u8, u8, u8) {
        (self.command.command_id(), self.r, self.g, self.b)
    }
}

// Methods brightness_command and transition_command were removed as they are unused

pub fn load_config(path: &str) -> Result<Config, Box<dyn std::error::Error>> {
    let contents = fs::read_to_string(path)?;
    let mut config: Config = serde_json::from_str(&contents)?;
    let default = Config::default();
    for (k, v) in default.presence_map {
        config.presence_map.entry(k).or_insert(v);
    }
    config.clamp_ranges();
    Ok(config)
}

/// Writes to a sibling temp file and renames over the target so a crash never leaves a truncated config.
pub fn save_config(path: &str, config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::to_string_pretty(config)?;
    let target = Path::new(path);
    let tmp = target.with_extension("json.tmp");
    fs::write(&tmp, json)?;
    if let Err(e) = fs::rename(&tmp, target) {
        let _ = fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_includes_all_presence_states() {
        let config = Config::default();
        assert!(config.presence_map.contains_key("Available"));
        assert!(config.presence_map.contains_key("AvailableIdle"));
        assert!(config.presence_map.contains_key("Busy"));
        assert!(config.presence_map.contains_key("BusyIdle"));
        assert!(config.presence_map.contains_key("Away"));
        assert!(config.presence_map.contains_key("BeRightBack"));
        assert!(config.presence_map.contains_key("DoNotDisturb"));
        assert!(config.presence_map.contains_key("Offline"));
        assert!(config.presence_map.contains_key("Unknown"));
    }

    #[test]
    fn test_load_config_merges_missing_defaults() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("partial_config.json");
        // Write a json missing AvailableIdle and BusyIdle
        let partial_json = r#"{
            "pollIntervalMs": 5000,
            "pingIntervalMs": 15000,
            "brightness": 191,
            "transitionDurationMs": 500,
            "presenceMap": {
                "Available": { "command": "SOLID", "r": 0, "g": 200, "b": 0 }
            }
        }"#;
        fs::write(&path, partial_json).unwrap();

        let loaded = load_config(path.to_str().unwrap()).unwrap();
        assert!(loaded.presence_map.contains_key("AvailableIdle"));
        assert!(loaded.presence_map.contains_key("BusyIdle"));
        assert_eq!(loaded.presence_map.get("Available").unwrap().command, LedMode::Solid);
    }

    #[test]
    fn test_load_config_tolerates_missing_fields_and_clamps_ranges() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("minimal_config.json");
        // Legacy "watchdog" key must be ignored, not rejected.
        fs::write(&path, r#"{ "pingIntervalMs": 0, "pollIntervalMs": 999999, "watchdog": { "command": "BREATHE_SLOW", "r": 255, "g": 255, "b": 255 } }"#).unwrap();

        let loaded = load_config(path.to_str().unwrap()).unwrap();
        assert_eq!(loaded.ping_interval_ms, MIN_PING_INTERVAL_MS);
        assert_eq!(loaded.poll_interval_ms, MAX_POLL_INTERVAL_MS);
        assert!(loaded.presence_map.contains_key("Offline"));
    }

    #[test]
    fn test_unknown_led_mode_is_rejected_at_load() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("typo_config.json");
        fs::write(&path, r#"{ "presenceMap": { "Busy": { "command": "BRETHE", "r": 1, "g": 2, "b": 3 } } }"#).unwrap();
        assert!(load_config(path.to_str().unwrap()).is_err());
    }

    #[test]
    fn test_led_mode_round_trips_as_protocol_names() {
        for mode in LedMode::ALL {
            let json = serde_json::to_string(&mode).unwrap();
            assert_eq!(json, format!("\"{}\"", mode.label()));
            assert_eq!(serde_json::from_str::<LedMode>(&json).unwrap(), mode);
        }
        assert_eq!(LedMode::Off.command_id(), 0x02);
        assert_eq!(LedMode::BreatheSlow.command_id(), 0x05);
    }

    #[test]
    fn test_save_config_is_atomic_and_leaves_no_temp_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("config.json");
        save_config(path.to_str().unwrap(), &Config::default()).unwrap();
        assert!(path.exists());
        assert!(!temp_dir.path().join("config.json.tmp").exists());
        load_config(path.to_str().unwrap()).unwrap();
    }
}
