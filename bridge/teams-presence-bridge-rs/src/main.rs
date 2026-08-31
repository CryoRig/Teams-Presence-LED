//#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod autostart;
mod config;
mod hid;
mod teams;
mod ui;
mod updater;
mod flasher;
mod update_ui;

use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::fs;
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
    pub firmware_version: Option<(u8, u8, u8, u8)>,
    pub update_available: bool,
}

fn main() -> eframe::Result<()> {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
    let lock_path = exe_dir.join(".bridge.lock");
    let mut open_opts = fs::OpenOptions::new();
    open_opts.write(true).create(true);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::fs::OpenOptionsExt;
        open_opts.share_mode(0); // Exclusive lock on Windows
    }

    let _single_instance_lock = match open_opts.open(&lock_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("[Bridge] Another instance of Teams Presence Bridge is ALREADY running in the background/system tray! ({})", e);
            eprintln!("[Bridge] To reopen the settings window, click the Teams icon in your Windows System Tray (bottom right near clock).");
            return Ok(());
        }
    };

    let config_path = exe_dir.join("config.json");
    let config_path_str = config_path.to_string_lossy().to_string();

    let config = if !config_path.exists() {
        let default_config = Config::default();
        if let Err(e) = config::save_config(&config_path_str, &default_config) {
            eprintln!("Failed to create default config at {}: {}", config_path_str, e);
        }
        default_config
    } else {
        match load_config(&config_path_str) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Failed to parse config from {}: {}", config_path_str, e);
                let backup_path = exe_dir.join("config.json.bak");
                match fs::rename(&config_path, &backup_path) {
                    Ok(_) => eprintln!("Backed up invalid config to {}", backup_path.to_string_lossy()),
                    Err(rename_err) => eprintln!("Failed to back up invalid config to {}: {}", backup_path.to_string_lossy(), rename_err),
                }

                let default_config = Config::default();
                if let Err(save_err) = config::save_config(&config_path_str, &default_config) {
                    eprintln!("Failed to write default config after parse error: {}", save_err);
                }
                default_config
            }
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

    let flash_pause_flag = Arc::new(AtomicBool::new(false));
    let background_flash_pause = flash_pause_flag.clone();
    let app_flash_pause = flash_pause_flag.clone();

    let bootloader_trigger = Arc::new(AtomicBool::new(false));
    let background_bootloader_trigger = bootloader_trigger.clone();
    let app_bootloader_trigger = bootloader_trigger.clone();

    let update_ui_state = Arc::new(Mutex::new(update_ui::UpdateUiState::new(
        semver::Version::parse(env!("CARGO_PKG_VERSION")).unwrap(),
    )));
    let app_update_ui_state = update_ui_state.clone();

    // Spawn background bridge thread
    thread::spawn(move || {
        run_bridge_loop(
            background_config,
            background_status,
            background_ctx,
            background_shutdown,
            background_flash_pause,
            background_bootloader_trigger,
        );
    });

    // Spawn background update check on startup
    let startup_update_state = update_ui_state.clone();
    let startup_status = app_status.clone();
    let startup_ctx = shared_ctx.clone();
    thread::spawn(move || {
        // Wait up to 10 seconds for the egui Context to be initialized
        let mut egui_ctx = None;
        for _ in 0..100 {
            if let Some(c) = startup_ctx.lock().unwrap().as_ref() {
                egui_ctx = Some(c.clone());
                break;
            }
            thread::sleep(std::time::Duration::from_millis(100));
        }

        // Run the update check
        if let Ok(latest) = updater::fetch_latest_release() {
            let mut s = startup_update_state.lock().unwrap();
            let fw_version_opt = s.firmware_current.clone();
            let res = updater::check_updates(&s.bridge_current, fw_version_opt.as_ref(), &latest);
            s.latest_release = Some(latest);
            s.bridge_update_available = res.bridge_update_available;
            s.firmware_update_available = res.firmware_update_available;
            
            startup_status.lock().unwrap().update_available = res.bridge_update_available || res.firmware_update_available;

            if let Some(c) = egui_ctx {
                c.request_repaint();
            }
        }
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
            Ok(Box::new(ui::TeamsBridgeApp::new(
                cc,
                shared_config,
                app_status,
                config_path_str,
                shutdown_flag,
                app_flash_pause,
                app_bootloader_trigger,
                app_update_ui_state,
            )))
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
    flash_pause_flag: Arc<AtomicBool>,
    bootloader_trigger: Arc<AtomicBool>,
) {
    let mut hid_manager: Option<HidManager> = None;
    let mut last_hid_init_attempt = Instant::now() - Duration::from_secs(5);
    let mut teams_client = TeamsClient::new();
    
    let mut previous_presence: Option<String> = None;
    let mut last_ping_time = Instant::now();
    let mut last_poll_time = Instant::now();
    let mut last_sent_brightness: Option<u8> = None; // Track to detect live slider changes
    let mut last_sent_transition: Option<u16> = None; // Track to detect live slider changes

    loop {
        let now = Instant::now();

        if hid_manager.is_none() && now.duration_since(last_hid_init_attempt) >= Duration::from_secs(5) {
            last_hid_init_attempt = now;
            match HidManager::new() {
                Ok(mut manager) => {
                    let connected = manager.connect();
                    if connected {
                        let c = config.lock().unwrap();
                        manager.send_brightness(c.brightness);
                        last_sent_brightness = Some(c.brightness);
                        std::thread::sleep(std::time::Duration::from_millis(10));
                        
                        manager.send_transition(c.transition_duration_ms);
                        last_sent_transition = Some(c.transition_duration_ms);
                        std::thread::sleep(std::time::Duration::from_millis(10));
                        
                        previous_presence = None; // Force re-send of presence color
                    }
                    hid_manager = Some(manager);
                }
                Err(e) => {
                    eprintln!("[Bridge] HID init failed, will retry: {}", e);
                }
            }
        }

        // Pause during firmware flash
        if flash_pause_flag.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(100));
            continue;
        }

        // Check if we need to put the device into bootloader mode
        if bootloader_trigger.load(Ordering::Relaxed) {
            if hid_manager.as_ref().is_some_and(|h| h.is_connected()) {
                eprintln!("[Bridge] Entering bootloader mode as requested...");
                if let Some(h) = hid_manager.as_mut() {
                    h.enter_bootloader();
                }
            }
            bootloader_trigger.store(false, Ordering::Relaxed);
            flash_pause_flag.store(true, Ordering::Relaxed);
            previous_presence = None; // Force re-send on reconnect
            thread::sleep(Duration::from_millis(100));
            continue;
        }
        // Fast tick for shutdown and UI responsiveness
        if shutdown_flag.load(Ordering::Relaxed) {
            if let Some(h) = hid_manager.as_mut() {
                h.send_color_command(0x02, 0, 0, 0); // CMD_OFF
            }
            break;
        }

        let (poll_interval, ping_interval, presence_map, watchdog, brightness, transition_duration_ms) = {
            let c = config.lock().unwrap();
            (c.poll_interval_ms, c.ping_interval_ms, c.presence_map.clone(), c.watchdog.clone(), c.brightness, c.transition_duration_ms)
        };

        // 1. Slow tick: Teams polling and Serial reconnection
        if now.duration_since(last_poll_time).as_millis() as u64 >= poll_interval {
            last_poll_time = now;

            // If not connected, drop context and retry next tick
            if let Some(h) = hid_manager.as_ref() {
                if !h.is_connected() {
                    hid_manager = None;
                    continue;
                }
            }

            // Query version if we are connected but don't have it yet
            if let Some(h) = hid_manager.as_mut()
                && h.is_connected() {
                    let has_version = status.lock().unwrap().firmware_version.is_some();
                    if !has_version
                        && let Some(ver) = h.query_firmware_version() {
                            eprintln!("[Bridge] Queried firmware version: v{}.{}.{} (variant: {})", ver.0, ver.1, ver.2, ver.3);
                            status.lock().unwrap().firmware_version = Some(ver);
                        }
                }

            let presence = teams_client.get_presence();
            if presence != previous_presence {
                if let Some(p) = &presence {
                    eprintln!("[Bridge] Presence changed to: {}", p);
                    let cmd_params = if let Some(c) = presence_map.get(p) {
                        eprintln!("[Bridge] Found mapped color for {}: {:?}", p, c);
                        c.to_hid_params()
                    } else if let Some(unknown) = presence_map.get("Unknown") {
                        eprintln!("[Bridge] Unknown presence '{}' not in presence_map, using 'Unknown' mapping", p);
                        unknown.to_hid_params()
                    } else {
                        eprintln!("[Bridge] Unknown presence '{}' and no 'Unknown' mapping found, sending watchdog command", p);
                        watchdog.to_hid_params()
                    };
                    if let Some(h) = hid_manager.as_mut() {
                        eprintln!("[Bridge] Sending HID Color Command -> CMD: {}, R: {}, G: {}, B: {}", cmd_params.0, cmd_params.1, cmd_params.2, cmd_params.3);
                        h.send_color_command(cmd_params.0, cmd_params.1, cmd_params.2, cmd_params.3);
                    }
                    last_ping_time = Instant::now(); // Presence command proves host is alive
                } else {
                    eprintln!("[Bridge] Presence changed to: None (Teams not running or no status detected)");
                }
                previous_presence = presence.clone();
            }
        }

        // 2. Fast tick updates: Live brightness update from UI
        if let Some(h) = hid_manager.as_mut()
            && h.is_connected() {
                if last_sent_brightness != Some(brightness) {
                    h.send_brightness(brightness);
                    last_sent_brightness = Some(brightness);
                }
                if last_sent_transition != Some(transition_duration_ms) {
                    h.send_transition(transition_duration_ms);
                    last_sent_transition = Some(transition_duration_ms);
                }
            }

        // 3. Ping tick
        if let Some(h) = hid_manager.as_mut()
            && h.is_connected() && now.duration_since(last_ping_time).as_millis() as u64 >= ping_interval {
                h.send_ping();
                last_ping_time = Instant::now();
            }

        // 4. Update UI Status
        let connected = hid_manager.as_ref().is_some_and(|h| h.is_connected());
        let parsing = teams_client.has_valid_log();

        let mut status_changed = false;
        {
            let mut s = status.lock().unwrap();
            if s.esp_connected != connected || s.teams_parsing != parsing {
                s.esp_connected = connected;
                s.teams_parsing = parsing;
                if !connected {
                    s.firmware_version = None;
                }
                status_changed = true;
            }
        }
        if status_changed
            && let Some(ctx) = ctx.lock().unwrap().as_ref() {
                ctx.request_repaint();
            }

        thread::sleep(Duration::from_millis(100));
    }
}
