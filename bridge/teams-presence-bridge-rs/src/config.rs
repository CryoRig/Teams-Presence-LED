use std::collections::HashMap;
use std::fs;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub poll_interval_ms: u64,
    pub ping_interval_ms: u64,
    #[serde(default = "default_brightness")]
    pub brightness: u8,
    #[serde(default = "default_transition")]
    pub transition_duration_ms: u16,
    pub presence_map: HashMap<String, ColorCommand>,
    pub watchdog: ColorCommand,
}

fn default_brightness() -> u8 {
    191 // ~75% of 255
}

fn default_transition() -> u16 {
    500
}

impl Default for Config {
    fn default() -> Self {
        let mut presence_map = HashMap::new();
        presence_map.insert("DoNotDisturb".into(), ColorCommand { command: "SOLID".into(), r: 200, g: 0, b: 0 });
        presence_map.insert("Away".into(), ColorCommand { command: "BREATHE_SLOW".into(), r: 255, g: 120, b: 0 });
        presence_map.insert("Busy".into(), ColorCommand { command: "SOLID".into(), r: 200, g: 0, b: 0 });
        presence_map.insert("BusyIdle".into(), ColorCommand { command: "BREATHE_SLOW".into(), r: 200, g: 0, b: 0 });
        presence_map.insert("BeRightBack".into(), ColorCommand { command: "BREATHE_SLOW".into(), r: 255, g: 80, b: 0 });
        presence_map.insert("Unknown".into(), ColorCommand { command: "BREATHE".into(), r: 80, g: 80, b: 80 });
        presence_map.insert("Available".into(), ColorCommand { command: "SOLID".into(), r: 0, g: 200, b: 0 });
        presence_map.insert("AvailableIdle".into(), ColorCommand { command: "BREATHE_SLOW".into(), r: 255, g: 120, b: 0 });
        presence_map.insert("Offline".into(), ColorCommand { command: "OFF".into(), r: 0, g: 0, b: 0 });

        Self {
            poll_interval_ms: 5000,
            ping_interval_ms: 15000,
            brightness: 191, // ~75%
            transition_duration_ms: 500,
            presence_map,
            watchdog: ColorCommand { command: "BREATHE_SLOW".into(), r: 255, g: 255, b: 255 },
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ColorCommand {
    pub command: String,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl ColorCommand {
    pub fn to_hid_params(&self) -> (u8, u8, u8, u8) {
        let cmd_id = match self.command.as_str() {
            "OFF" => 0x02,
            "SOLID" => 0x03,
            "BREATHE" => 0x04,
            "BREATHE_SLOW" => 0x05,
            _ => 0x02, // fallback to OFF
        };
        (cmd_id, self.r, self.g, self.b)
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
    Ok(config)
}

pub fn save_config(path: &str, config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::to_string_pretty(config)?;
    fs::write(path, json)?;
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
            },
            "watchdog": { "command": "BREATHE_SLOW", "r": 255, "g": 255, "b": 255 }
        }"#;
        fs::write(&path, partial_json).unwrap();

        let loaded = load_config(path.to_str().unwrap()).unwrap();
        assert!(loaded.presence_map.contains_key("AvailableIdle"));
        assert!(loaded.presence_map.contains_key("BusyIdle"));
        assert_eq!(loaded.presence_map.get("Available").unwrap().command, "SOLID");
    }
}
