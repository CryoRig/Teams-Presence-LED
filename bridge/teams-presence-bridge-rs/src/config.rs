use std::collections::HashMap;
use std::fs;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub com_port: String,
    pub poll_interval_ms: u64,
    pub ping_interval_ms: u64,
    pub presence_map: HashMap<String, ColorCommand>,
    pub watchdog: ColorCommand,
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
        format!("{}:{},{},{}\n", self.command, self.r, self.g, self.b)
    }
}

pub fn load_config(path: &str) -> Result<Config, Box<dyn std::error::Error>> {
    let contents = fs::read_to_string(path)?;
    let config: Config = serde_json::from_str(&contents)?;
    Ok(config)
}
