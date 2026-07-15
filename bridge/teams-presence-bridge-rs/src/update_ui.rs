use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use eframe::egui;
use semver::Version;
use crate::updater;
use crate::flasher::{flash_firmware, FlashStage};

pub struct UpdateUiState {
    pub show_window: bool,
    pub bridge_current: Version,
    pub firmware_current: Option<Version>,
    pub latest_release: Option<updater::ReleaseInfo>,
    pub bridge_update_available: bool,
    pub firmware_update_available: bool,
    pub flash_in_progress: bool,
    pub flash_stage: Option<FlashStage>,
    pub checking: bool,
    pub error_message: Option<String>,
}

impl UpdateUiState {
    pub fn new(bridge_current: Version) -> Self {
        Self {
            show_window: false,
            bridge_current,
            firmware_current: None,
            latest_release: None,
            bridge_update_available: false,
            firmware_update_available: false,
            flash_in_progress: false,
            flash_stage: None,
            checking: false,
            error_message: None,
        }
    }
}

pub fn render(
    ctx: &egui::Context,
    state: &Arc<Mutex<UpdateUiState>>,
    flash_pause_flag: &Arc<AtomicBool>,
    bootloader_trigger: &Arc<AtomicBool>,
    esp_connected: bool,
) {
    let mut show_win = {
        let s = state.lock().unwrap();
        s.show_window
    };

    if !show_win {
        return;
    }

    let flash_in_progress = {
        let s = state.lock().unwrap();
        s.flash_in_progress
    };

    // If flash is in progress, do not show the close button, to prevent user from closing during flash
    let mut window = egui::Window::new("Software Updates")
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]);

    if !flash_in_progress {
        window = window.open(&mut show_win);
    }

    window.show(ctx, |ui| {
        let mut check_now = false;
        let mut flash_now = false;

        {
            let s = state.lock().unwrap();
            
            ui.vertical(|ui| {
                ui.heading("Teams Presence Bridge Updates");
                ui.add_space(5.0);

                // --- Bridge Section ---
                ui.group(|ui| {
                    ui.label("Bridge Application");
                    ui.horizontal(|ui| {
                        ui.label(format!("Current version: v{}", s.bridge_current));
                        if s.bridge_update_available {
                            ui.colored_label(egui::Color32::from_rgb(0, 180, 0), "⬆ Update Available!");
                        }
                    });

                    if let Some(ref latest) = s.latest_release {
                        ui.label(format!("Latest version: v{}", latest.version));
                        if s.bridge_update_available {
                            ui.add_space(5.0);
                            if ui.button("Open GitHub Release Page").clicked() {
                                let _ = open::that(&latest.html_url);
                            }
                        }
                    }
                });

                ui.add_space(10.0);

                // --- Firmware Section ---
                ui.group(|ui| {
                    ui.label("ESP32 Firmware");
                    ui.horizontal(|ui| {
                        match &s.firmware_current {
                            Some(v) => ui.label(format!("Current version: v{}", v)),
                            None => ui.label("Current version: Unknown (ESP32 disconnected)"),
                        };
                        if s.firmware_update_available {
                            ui.colored_label(egui::Color32::from_rgb(0, 180, 0), "⬆ Update Available!");
                        }
                    });

                    if let Some(ref latest) = s.latest_release {
                        if latest.firmware_download_url.is_some() {
                            if s.firmware_update_available || s.firmware_current.is_none() {
                                ui.add_space(5.0);
                                ui.horizontal(|ui| {
                                    if ui.add_enabled(!s.flash_in_progress, egui::Button::new("Update Firmware")).clicked() {
                                        flash_now = true;
                                    }
                                    if s.firmware_current.is_none() {
                                        ui.label("(Ensure device is connected)");
                                    }
                                });
                            } else {
                                ui.label("Firmware is up to date.");
                            }
                        } else {
                            ui.label("No firmware binary in latest release.");
                        }
                    }
                });

                // --- Flash Status and Progress ---
                if s.flash_in_progress {
                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(5.0);
                    
                    let stage_text = match &s.flash_stage {
                        Some(FlashStage::Connecting) => "Downloading firmware...".to_string(),
                        Some(FlashStage::Erasing) => "Connecting & Erasing flash...".to_string(),
                        Some(FlashStage::Flashing { percent }) => format!("Flashing firmware... {}%", percent),
                        Some(FlashStage::Verifying) => "Verifying flash...".to_string(),
                        Some(FlashStage::Resetting) => "Resetting ESP32 device...".to_string(),
                        Some(FlashStage::Done) => "Done!".to_string(),
                        Some(FlashStage::Error(err)) => format!("Error: {}", err),
                        None => "Initializing...".to_string(),
                    };

                    ui.label(stage_text);

                    let progress = match &s.flash_stage {
                        Some(FlashStage::Flashing { percent }) => *percent as f32 / 100.0,
                        Some(FlashStage::Done) => 1.0,
                        _ => 0.0,
                    };

                    ui.add(egui::ProgressBar::new(progress).show_percentage());
                }

                // --- Error message ---
                if let Some(ref err) = s.error_message {
                    ui.add_space(10.0);
                    ui.colored_label(egui::Color32::from_rgb(220, 0, 0), err);
                }

                ui.add_space(15.0);
                ui.separator();
                ui.add_space(5.0);

                ui.horizontal(|ui| {
                    if ui.add_enabled(!s.checking && !s.flash_in_progress, egui::Button::new("Check for Updates")).clicked() {
                        check_now = true;
                    }
                    if s.checking {
                        ui.spinner();
                    }
                });
            });

        }

        if check_now {
            trigger_update_check(state.clone(), ctx.clone());
        }

        if flash_now {
            start_firmware_update(
                state.clone(),
                flash_pause_flag.clone(),
                bootloader_trigger.clone(),
                esp_connected,
                ctx.clone(),
            );
        }
    });

    if !flash_in_progress {
        let mut s = state.lock().unwrap();
        s.show_window = show_win;
    }
}

pub fn trigger_update_check(state: Arc<Mutex<UpdateUiState>>, ctx: egui::Context) {
    {
        let mut s = state.lock().unwrap();
        s.checking = true;
        s.error_message = None;
    }
    ctx.request_repaint();

    thread::spawn(move || {
        match updater::fetch_latest_release() {
            Ok(latest) => {
                let mut s = state.lock().unwrap();
                let fw_version_opt = s.firmware_current.clone();
                let res = updater::check_updates(&s.bridge_current, fw_version_opt.as_ref(), &latest);
                s.latest_release = Some(latest);
                s.bridge_update_available = res.bridge_update_available;
                s.firmware_update_available = res.firmware_update_available;
                s.checking = false;
            }
            Err(e) => {
                let mut s = state.lock().unwrap();
                s.error_message = Some(format!("Failed to fetch release: {}", e));
                s.checking = false;
            }
        }
        ctx.request_repaint();
    });
}

fn start_firmware_update(
    state: Arc<Mutex<UpdateUiState>>,
    flash_pause_flag: Arc<AtomicBool>,
    bootloader_trigger: Arc<AtomicBool>,
    esp_connected: bool,
    ctx: egui::Context,
) {
    let latest_release = {
        let s = state.lock().unwrap();
        s.latest_release.clone()
    };

    let firmware_url = match latest_release.and_then(|r| r.firmware_download_url) {
        Some(url) => url,
        None => {
            let mut s = state.lock().unwrap();
            s.error_message = Some("No firmware binary found in latest release.".to_string());
            return;
        }
    };

    {
        let mut s = state.lock().unwrap();
        s.flash_in_progress = true;
        s.flash_stage = Some(FlashStage::Connecting);
        s.error_message = None;
    }
    ctx.request_repaint();

    let state_clone = state.clone();
    let ctx_clone = ctx.clone();

    thread::spawn(move || {
        // Step 1: Download firmware binary
        let fw_path = match updater::download_firmware(&firmware_url) {
            Ok(path) => path,
            Err(e) => {
                let mut s = state_clone.lock().unwrap();
                s.flash_in_progress = false;
                s.flash_stage = None;
                s.error_message = Some(format!("Download failed: {}", e));
                ctx_clone.request_repaint();
                return;
            }
        };

        // Step 2: Trigger bootloader mode (or just proceed if already disconnected)
        if esp_connected {
            bootloader_trigger.store(true, Ordering::Relaxed);

            // Wait for bridge loop to pause
            let start = std::time::Instant::now();
            let mut ok = false;
            while start.elapsed().as_secs() < 5 {
                if flash_pause_flag.load(Ordering::Relaxed) {
                    ok = true;
                    break;
                }
                thread::sleep(std::time::Duration::from_millis(50));
            }

            if !ok {
                let mut s = state_clone.lock().unwrap();
                s.flash_in_progress = false;
                s.flash_stage = None;
                s.error_message = Some("Failed to enter bootloader mode (bridge timed out).".to_string());
                let _ = std::fs::remove_file(fw_path);
                ctx_clone.request_repaint();
                return;
            }
        } else {
            // Not connected via HID, so we assume it might already be in bootloader mode.
            // Directly pause the bridge loop and proceed.
            flash_pause_flag.store(true, Ordering::Relaxed);
        }

        // Wait 2s for USB CDC port to enumerate
        thread::sleep(std::time::Duration::from_millis(2000));

        // Step 3: Flash the firmware
        let state_cb = state_clone.clone();
        let ctx_cb = ctx_clone.clone();
        let flash_res = flash_firmware(&fw_path, move |stage| {
            let mut s = state_cb.lock().unwrap();
            s.flash_stage = Some(stage);
            ctx_cb.request_repaint();
        });

        match flash_res {
            Ok(_) => {
                // Wait 3s for ESP32 to reboot and CDC/HID to re-enumerate
                thread::sleep(std::time::Duration::from_millis(3000));

                let mut s = state_clone.lock().unwrap();
                s.flash_in_progress = false;
                s.flash_stage = None;
                s.firmware_update_available = false; // Successfully flashed
                // We don't know the exact firmware version until it re-enumerates and bridge loop queries it,
                // so we will let the bridge loop update s.firmware_current.
            }
            Err(e) => {
                let mut s = state_clone.lock().unwrap();
                s.flash_in_progress = false;
                s.flash_stage = None;
                s.error_message = Some(format!("Flash failed: {}", e));
            }
        }

        // Resume bridge loop
        flash_pause_flag.store(false, Ordering::Relaxed);
        ctx_clone.request_repaint();
    });
}
