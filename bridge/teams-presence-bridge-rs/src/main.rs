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

#[derive(Clone, Default, PartialEq, Debug)]
pub struct AppStatus {
    pub esp_connected: bool,
    pub esp_port: Option<String>,
    pub teams_parsing: bool,
}

fn main() -> eframe::Result<()> {
    let config = match load_config("config.json") {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to load config, generating default: {}", e);
            let default_config = Config::default();
            let _ = config::save_config("config.json", &default_config);
            default_config
        }
    };

    let shared_config = Arc::new(Mutex::new(config));
    let background_config = shared_config.clone();
    
    let app_status = Arc::new(Mutex::new(AppStatus::default()));
    let background_status = app_status.clone();
    
    let shared_ctx: Arc<Mutex<Option<egui::Context>>> = Arc::new(Mutex::new(None));
    let background_ctx = shared_ctx.clone();

    // Spawn background bridge thread
    thread::spawn(move || {
        run_bridge_loop(background_config, background_status, background_ctx);
    });

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_visible(false) // Start hidden
            .with_taskbar(false) // Hide from taskbar when hidden
            .with_inner_size([450.0, 500.0])
            .with_title("Teams Presence Bridge Settings"),
        ..Default::default()
    };

    // Run the eframe app
    eframe::run_native(
        "Teams Presence Bridge Settings",
        options,
        Box::new(move |cc| {
            *shared_ctx.lock().unwrap() = Some(cc.egui_ctx.clone());
            Ok(Box::new(ui::TeamsBridgeApp::new(cc, shared_config, app_status)))
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

fn run_bridge_loop(config: Arc<Mutex<Config>>, status: Arc<Mutex<AppStatus>>, ctx: Arc<Mutex<Option<egui::Context>>>) {
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

        let connected = serial_manager.is_connected();
        let port = serial_manager.get_port_name();
        let parsing = teams_client.has_valid_log();

        let mut status_changed = false;
        {
            let mut s = status.lock().unwrap();
            if s.esp_connected != connected || s.esp_port != port || s.teams_parsing != parsing {
                s.esp_connected = connected;
                s.esp_port = port;
                s.teams_parsing = parsing;
                status_changed = true;
            }
        }
        if status_changed {
            if let Some(ctx) = ctx.lock().unwrap().as_ref() {
                ctx.request_repaint();
            }
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
