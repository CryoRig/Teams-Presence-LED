#![windows_subsystem = "windows"] // Hides the console window on Windows

mod config;
mod serial;
mod teams;
mod ui;

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use config::{load_config, Config};
use serial::SerialManager;
use teams::TeamsClient;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let config = match load_config("config.json") {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to load config: {}", e);
            // In a real app we might want to generate a default config, but for now we exit.
            return Ok(());
        }
    };

    let shared_config = Arc::new(Mutex::new(config));
    let background_config = shared_config.clone();

    // Spawn background bridge thread
    thread::spawn(move || {
        run_bridge_loop(background_config);
    });

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_visible(true) // Keep true so eframe's event loop processes repaints
            .with_position([-10000.0, -10000.0]) // Start completely off-screen
            .with_taskbar(false) // Hide from taskbar initially
            .with_inner_size([450.0, 500.0])
            .with_title("Teams Presence Bridge Settings"),
        ..Default::default()
    };

    // Run the eframe app
    eframe::run_native(
        "Teams Presence Bridge Settings",
        options,
        Box::new(move |cc| {
            Ok(Box::new(ui::TeamsBridgeApp::new(cc, shared_config)))
        }),
    )?;
    Ok(())
}

pub fn create_dummy_icon() -> tray_icon::Icon {
    let width = 32;
    let height = 32;
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for _ in 0..(width * height) {
        rgba.extend_from_slice(&[0, 120, 215, 255]); // Teams Blue
    }
    tray_icon::Icon::from_rgba(rgba, width, height).unwrap()
}

fn run_bridge_loop(config: Arc<Mutex<Config>>) {
    let mut serial_manager = SerialManager::new();
    let mut teams_client = TeamsClient::new();
    
    let mut previous_presence: Option<String> = None;
    let mut last_ping_time = Instant::now();

    // Initial connection based on initial config
    let initial_port = config.lock().unwrap().com_port.clone();
    serial_manager.connect(&initial_port);

    loop {
        let (current_com_port, poll_interval, ping_interval, presence_map, watchdog) = {
            let c = config.lock().unwrap();
            (c.com_port.clone(), c.poll_interval_ms, c.ping_interval_ms, c.presence_map.clone(), c.watchdog.clone())
        };

        // Try reconnect if disconnected or if port changed
        if !serial_manager.is_connected() || serial_manager.get_port_name() != Some(current_com_port.clone()) {
            serial_manager.connect(&current_com_port);
        }

        let presence = teams_client.get_presence();
        if presence != previous_presence {
            if let Some(p) = &presence {
                let cmd = if let Some(c) = presence_map.get(p) {
                    c.to_serial_command()
                } else {
                    // Unknown presence map
                    watchdog.to_serial_command()
                };
                serial_manager.send_command(&cmd);
            }
            previous_presence = presence.clone();
        }

        if serial_manager.is_connected() && last_ping_time.elapsed().as_millis() as u64 >= ping_interval {
            serial_manager.send_ping();
            last_ping_time = Instant::now();
        }

        thread::sleep(Duration::from_millis(poll_interval));
    }
}
