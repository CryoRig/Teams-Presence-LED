#![windows_subsystem = "windows"] // Hides the console window on Windows

mod config;
mod hid;
mod teams;
mod ui;

use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use std::path::PathBuf;
use config::{load_config, Config};
use hid::HidManager;
use teams::TeamsClient;
use eframe::egui;

#[derive(Clone, Default, PartialEq, Debug)]
pub struct AppStatus {
    pub esp_connected: bool,
    pub teams_parsing: bool,
}

fn main() -> eframe::Result<()> {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
    let config_path = exe_dir.join("config.json");
    let config_path_str = config_path.to_string_lossy().to_string();

    let config = match load_config(&config_path_str) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to load config from {}, generating default: {}", config_path_str, e);
            let default_config = Config::default();
            let _ = config::save_config(&config_path_str, &default_config);
            default_config
        }
    };

    let shared_config = Arc::new(Mutex::new(config));
    let background_config = shared_config.clone();
    
    let app_status = Arc::new(Mutex::new(AppStatus::default()));
    let background_status = app_status.clone();
    
    let shared_ctx: Arc<Mutex<Option<egui::Context>>> = Arc::new(Mutex::new(None));
    let background_ctx = shared_ctx.clone();

    let shutdown_flag = Arc::new(AtomicBool::new(false));
    let background_shutdown = shutdown_flag.clone();

    // Spawn background bridge thread
    thread::spawn(move || {
        run_bridge_loop(background_config, background_status, background_ctx, background_shutdown);
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
            Ok(Box::new(ui::TeamsBridgeApp::new(cc, shared_config, app_status, config_path_str, shutdown_flag)))
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

fn run_bridge_loop(
    config: Arc<Mutex<Config>>,
    status: Arc<Mutex<AppStatus>>,
    ctx: Arc<Mutex<Option<egui::Context>>>,
    shutdown_flag: Arc<AtomicBool>,
) {
    let mut hid_manager = HidManager::new();
    let mut teams_client = TeamsClient::new();
    
    let mut previous_presence: Option<String> = None;
    let mut last_ping_time = Instant::now();
    let mut last_poll_time = Instant::now();
    let mut last_sent_brightness: Option<u8> = None; // Track to detect live slider changes
    let mut last_sent_transition: Option<u16> = None; // Track to detect live slider changes

    hid_manager.connect();

    loop {
        // Fast tick for shutdown and UI responsiveness
        if shutdown_flag.load(Ordering::Relaxed) {
            hid_manager.send_color_command(0x02, 0, 0, 0); // CMD_OFF
            break;
        }

        let (poll_interval, ping_interval, presence_map, watchdog, brightness, transition_duration_ms) = {
            let c = config.lock().unwrap();
            (c.poll_interval_ms, c.ping_interval_ms, c.presence_map.clone(), c.watchdog.clone(), c.brightness, c.transition_duration_ms)
        };

        let now = Instant::now();

        // 1. Slow tick: Teams polling and Serial reconnection
        if now.duration_since(last_poll_time).as_millis() as u64 >= poll_interval {
            last_poll_time = now;

            // Try reconnect if disconnected
            let was_disconnected = !hid_manager.is_connected();
            if was_disconnected {
                hid_manager.connect();
                // On fresh connection, send brightness and force re-send of current presence
                if hid_manager.is_connected() {
                    hid_manager.send_brightness(brightness);
                    last_sent_brightness = Some(brightness);
                    hid_manager.send_transition(transition_duration_ms);
                    last_sent_transition = Some(transition_duration_ms);
                    previous_presence = None; // Force re-send of presence color
                }
            }

            let presence = teams_client.get_presence();
            if presence != previous_presence {
                if let Some(p) = &presence {
                    let cmd_params = if let Some(c) = presence_map.get(p) {
                        c.to_hid_params()
                    } else {
                        eprintln!("[Bridge] Unknown presence '{}' not in presence_map, sending watchdog command", p);
                        watchdog.to_hid_params()
                    };
                    hid_manager.send_color_command(cmd_params.0, cmd_params.1, cmd_params.2, cmd_params.3);
                    last_ping_time = Instant::now(); // Presence command proves host is alive
                }
                previous_presence = presence.clone();
            }
        }

        // 2. Fast tick updates: Live brightness update from UI
        if hid_manager.is_connected() {
            if last_sent_brightness != Some(brightness) {
                hid_manager.send_brightness(brightness);
                last_sent_brightness = Some(brightness);
            }
            if last_sent_transition != Some(transition_duration_ms) {
                hid_manager.send_transition(transition_duration_ms);
                last_sent_transition = Some(transition_duration_ms);
            }
        }

        // 3. Ping tick
        if hid_manager.is_connected() && now.duration_since(last_ping_time).as_millis() as u64 >= ping_interval {
            hid_manager.send_ping();
            last_ping_time = Instant::now();
        }

        // 4. Update UI Status
        let connected = hid_manager.is_connected();
        let parsing = teams_client.has_valid_log();

        let mut status_changed = false;
        {
            let mut s = status.lock().unwrap();
            if s.esp_connected != connected || s.teams_parsing != parsing {
                s.esp_connected = connected;
                s.teams_parsing = parsing;
                status_changed = true;
            }
        }
        if status_changed {
            if let Some(ctx) = ctx.lock().unwrap().as_ref() {
                ctx.request_repaint();
            }
        }

        thread::sleep(Duration::from_millis(100));
    }
}
