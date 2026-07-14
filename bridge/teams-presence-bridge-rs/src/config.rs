use std::collections::HashMap;
use std::fs;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub com_port: String,
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
        presence_map.insert("BeRightBack".into(), ColorCommand { command: "BREATHE_SLOW".into(), r: 255, g: 80, b: 0 });
        presence_map.insert("Unknown".into(), ColorCommand { command: "BREATHE".into(), r: 80, g: 80, b: 80 });
        presence_map.insert("Available".into(), ColorCommand { command: "SOLID".into(), r: 0, g: 200, b: 0 });
        presence_map.insert("Offline".into(), ColorCommand { command: "OFF".into(), r: 0, g: 0, b: 0 });

        Self {
            com_port: "AUTO".to_string(),
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
    pub fn to_serial_command(&self) -> String {
        if self.command == "OFF" {
            "OFF\n".to_string()
        } else {
            format!("{}:{},{},{}\n", self.command, self.r, self.g, self.b)
        }
    }
}

// Methods brightness_command and transition_command were removed as they are unused

pub fn load_config(path: &str) -> Result<Config, Box<dyn std::error::Error>> {
    let contents = fs::read_to_string(path)?;
    let config: Config = serde_json::from_str(&contents)?;
    Ok(config)
}

pub fn save_config(path: &str, config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::to_string_pretty(config)?;
    fs::write(path, json)?;
    Ok(())
}
