#![windows_subsystem = "windows"] // Hides the console window on Windows

mod config;
mod serial;
mod teams;

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tray_item::{IconSource, TrayItem};
use config::load_config;
use serial::SerialManager;
use teams::TeamsClient;

fn main() {
    let config = match load_config("config.json") {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to load config: {}", e);
            return;
        }
    };

    // Setup tray icon
    let mut tray = TrayItem::new(
        "Teams Presence Bridge",
        IconSource::Resource("tray-icon"), // Requires a .rc file, or we can use IconSource::Resource("") which might fallback. Actually for testing we can just use IconSource::Resource("IDI_ICON1") if we add resources, but tray-item allows loading from memory or just text? tray-item Windows implementation expects a resource ID. Let's use a blank or default.
    ).unwrap();

    let quit_flag = Arc::new(Mutex::new(false));
    
    let quit_clone = quit_flag.clone();
    tray.add_menu_item("Quit", move || {
        *quit_clone.lock().unwrap() = true;
    }).unwrap();

    // Main bridge loop state
    let mut serial_manager = SerialManager::new();
    let mut teams_client = TeamsClient::new();
    
    let mut previous_presence: Option<String> = None;
    let mut last_ping_time = Instant::now();

    // Initial serial connection
    serial_manager.connect(&config.com_port);

    loop {
        if *quit_flag.lock().unwrap() {
            break;
        }

        // Try reconnect if disconnected
        if !serial_manager.is_connected() {
            serial_manager.connect(&config.com_port);
        }

        let presence = teams_client.get_presence();
        if presence != previous_presence {
            if let Some(p) = &presence {
                let cmd = if let Some(c) = config.presence_map.get(p) {
                    c.to_serial_command()
                } else {
                    // Unknown presence map
                    "BREATHE:80,80,80\n".to_string()
                };
                serial_manager.send_command(&cmd);
            }
            previous_presence = presence;
        }

        if serial_manager.is_connected() && last_ping_time.elapsed().as_millis() as u64 >= config.ping_interval_ms {
            serial_manager.send_ping();
            last_ping_time = Instant::now();
        }

        thread::sleep(Duration::from_millis(config.poll_interval_ms));
    }
}

