use eframe::egui;
use std::sync::{Arc, Mutex};
use crate::config::{Config, ColorCommand};

pub struct TeamsBridgeApp {
    config: Arc<Mutex<Config>>,
    local_config: Config,
    #[allow(dead_code)]
    tray_icon: tray_icon::TrayIcon,
    is_first_frame: bool,
    status: Arc<Mutex<crate::AppStatus>>,
    last_status: crate::AppStatus,
    esp_status_item: tray_icon::menu::MenuItem,
    teams_status_item: tray_icon::menu::MenuItem,
    update_item: tray_icon::menu::MenuItem,
    config_path: String,
    shutdown_flag: Arc<std::sync::atomic::AtomicBool>,
    autostart_enabled: bool,
    flash_pause_flag: Arc<std::sync::atomic::AtomicBool>,
    bootloader_trigger: Arc<std::sync::atomic::AtomicBool>,
    update_ui_state: Arc<Mutex<crate::update_ui::UpdateUiState>>,
}

impl TeamsBridgeApp {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        config: Arc<Mutex<Config>>,
        status: Arc<Mutex<crate::AppStatus>>,
        config_path: String,
        shutdown_flag: Arc<std::sync::atomic::AtomicBool>,
        flash_pause_flag: Arc<std::sync::atomic::AtomicBool>,
        bootloader_trigger: Arc<std::sync::atomic::AtomicBool>,
        update_ui_state: Arc<Mutex<crate::update_ui::UpdateUiState>>,
    ) -> Self {
        let local_config = config.lock().unwrap().clone();

        let tray_menu = tray_icon::menu::Menu::new();
        let esp_status_item = tray_icon::menu::MenuItem::with_id("esp_status", "🔴 ESP32: Disconnected", true, None);
        let teams_status_item = tray_icon::menu::MenuItem::with_id("teams_status", "🔴 Teams: Log Not Found", true, None);
        let update_item = tray_icon::menu::MenuItem::with_id("updates", "Check for Updates", true, None);
        let quit_i = tray_icon::menu::MenuItem::with_id("quit", "Quit", true, None);
        let _ = tray_menu.append_items(&[
            &esp_status_item,
            &teams_status_item,
            &tray_icon::menu::PredefinedMenuItem::separator(),
            &update_item,
            &tray_icon::menu::PredefinedMenuItem::separator(),
            &quit_i,
        ]);

        let tray_icon = tray_icon::TrayIconBuilder::new()
            .with_menu(Box::new(tray_menu))
            .with_menu_on_left_click(false)
            .with_tooltip("Teams Presence Bridge")
            .with_icon(crate::create_dummy_icon())
            .build()
            .unwrap();
            
        let ctx = cc.egui_ctx.clone();
        std::thread::spawn(move || {
            let receiver = tray_icon::TrayIconEvent::receiver();
            while let Ok(event) = receiver.recv() {
                if let tray_icon::TrayIconEvent::Click { button: tray_icon::MouseButton::Left, button_state: tray_icon::MouseButtonState::Up, .. } = event {
                    // Show the window
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                    ctx.request_repaint();
                }
            }
        });

        let ctx_menu = cc.egui_ctx.clone();
        let shutdown_quit = shutdown_flag.clone();
        let menu_update_state = update_ui_state.clone();
        std::thread::spawn(move || {
            let receiver = tray_icon::menu::MenuEvent::receiver();
            while let Ok(event) = receiver.recv() {
                if event.id.0 == "quit" {
                    shutdown_quit.store(true, std::sync::atomic::Ordering::Relaxed);
                    ctx_menu.send_viewport_cmd(egui::ViewportCommand::Close);
                } else if event.id.0 == "updates" {
                    menu_update_state.lock().unwrap().show_window = true;
                    ctx_menu.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx_menu.send_viewport_cmd(egui::ViewportCommand::Focus);
                    ctx_menu.request_repaint();
                }
            }
        });
            
        Self {
            config,
            local_config,
            tray_icon,
            is_first_frame: true,
            status,
            last_status: crate::AppStatus::default(),
            esp_status_item,
            teams_status_item,
            update_item,
            config_path,
            shutdown_flag,
            autostart_enabled: crate::autostart::is_autostart_enabled(),
            flash_pause_flag,
            bootloader_trigger,
            update_ui_state,
        }
    }
}

impl eframe::App for TeamsBridgeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        // --- Status Update Logic ---
        let current_status = self.status.lock().unwrap().clone();
        
        // Sync firmware version from status to update state
        if let Some(fw_ver_tuple) = current_status.firmware_version {
            if let Ok(fw_semver) = semver::Version::parse(&format!("{}.{}.{}", fw_ver_tuple.0, fw_ver_tuple.1, fw_ver_tuple.2)) {
                let mut state = self.update_ui_state.lock().unwrap();
                if state.firmware_current.as_ref() != Some(&fw_semver) {
                    state.firmware_current = Some(fw_semver.clone());
                    if let Some(ref latest) = state.latest_release {
                        let res = crate::updater::check_updates(&state.bridge_current, Some(&fw_semver), latest);
                        state.firmware_update_available = res.firmware_update_available;
                        
                        let update_avail = state.bridge_update_available || res.firmware_update_available;
                        self.status.lock().unwrap().update_available = update_avail;
                    }
                }
            }
        } else {
            let mut state = self.update_ui_state.lock().unwrap();
            if state.firmware_current.is_some() {
                state.firmware_current = None;
                state.firmware_update_available = false;
                self.status.lock().unwrap().update_available = state.bridge_update_available;
            }
        }

        let current_status = self.status.lock().unwrap().clone(); // Re-read since we might have updated update_available
        if current_status != self.last_status {
            if current_status.esp_connected {
                self.esp_status_item.set_text("🟢 ESP32: Connected (USB HID)");
            } else {
                self.esp_status_item.set_text("🔴 ESP32: Disconnected");
            }

            if current_status.teams_parsing {
                self.teams_status_item.set_text("🟢 Teams: Log Parsing Active");
            } else {
                self.teams_status_item.set_text("🔴 Teams: Log Not Found");
            }

            if current_status.update_available {
                self.update_item.set_text("⬆ Update Available!");
            } else {
                self.update_item.set_text("Check for Updates");
            }

            self.last_status = current_status.clone();
        }
        // ---------------------------
        
        if self.is_first_frame {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            self.is_first_frame = false;
        }

        // Intercept window close to hide instead of quit, unless we are genuinely shutting down
        if ctx.input(|i| i.viewport().close_requested()) && !self.shutdown_flag.load(std::sync::atomic::Ordering::Relaxed) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            // Hide it
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }

        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("Teams Presence Bridge Settings");

            ui.add_space(10.0);
            egui::Grid::new("settings_grid")
                .num_columns(2)
                .spacing([20.0, 10.0])
                .min_col_width(120.0)
                .show(ui, |ui| {
                    ui.label("Run on Windows Startup:");
                    if ui.checkbox(&mut self.autostart_enabled, "Enable Autostart").changed() {
                        if let Err(e) = crate::autostart::set_autostart(self.autostart_enabled) {
                            eprintln!("Failed to update autostart setting: {}", e);
                            // Revert on failure
                            self.autostart_enabled = !self.autostart_enabled;
                        }
                    }
                    ui.end_row();

                    ui.label("Poll Interval (ms):");
                    ui.add(egui::DragValue::new(&mut self.local_config.poll_interval_ms).speed(100.0).range(100..=10000));
                    ui.end_row();

                    ui.label("Ping Interval (ms):");
                    ui.add(egui::DragValue::new(&mut self.local_config.ping_interval_ms).speed(1000.0).range(1000..=60000));
                    ui.end_row();
                });

            ui.add_space(20.0);
            ui.heading("Brightness & Transitions");
            ui.add_space(5.0);

            let brightness_pct = (self.local_config.brightness as f32 / 255.0 * 100.0).round() as u32;
            let mut brightness_slider_val = brightness_pct as i32;
            let slider = egui::Slider::new(&mut brightness_slider_val, 0..=100)
                .suffix("%")
                .text("Global LED Brightness");
            if ui.add(slider).changed() {
                let new_brightness = (brightness_slider_val as f32 * 255.0 / 100.0).round() as u8;
                self.local_config.brightness = new_brightness;
                // Live preview: immediately push brightness to shared config
                // so the bridge loop picks it up and sends to ESP
                self.config.lock().unwrap().brightness = new_brightness;
            }

            let mut transition_val = self.local_config.transition_duration_ms as i32;
            let transition_slider = egui::Slider::new(&mut transition_val, 0..=2000)
                .step_by(50.0)
                .suffix(" ms")
                .text("Transition Duration (0 = Disabled)");
            if ui.add(transition_slider).changed() {
                let new_transition = transition_val as u16;
                self.local_config.transition_duration_ms = new_transition;
                self.config.lock().unwrap().transition_duration_ms = new_transition;
            }

            ui.add_space(20.0);
            ui.heading("Presence Mapping");
            ui.add_space(10.0);
            
            // Sort keys to maintain stable order
            let mut keys: Vec<String> = self.local_config.presence_map.keys().cloned().collect();
            keys.sort();

            egui::ScrollArea::vertical().max_height(250.0).show(ui, |ui| {
                egui::Grid::new("presence_mapping_grid")
                    .num_columns(2)
                    .spacing([20.0, 10.0])
                    .min_col_width(120.0)
                    .show(ui, |ui| {
                        for key in keys {
                            ui.label(format!("{}:", key));
                            if let Some(cmd) = self.local_config.presence_map.get_mut(&key) {
                                ui.horizontal(|ui| {
                                    render_color_command(ui, cmd, &key);
                                });
                            }
                            ui.end_row();
                        }
                    });
            });

            ui.add_space(10.0);
            egui::Grid::new("watchdog_grid")
                .num_columns(2)
                .spacing([20.0, 10.0])
                .min_col_width(120.0)
                .show(ui, |ui| {
                    ui.label("Watchdog:");
                    ui.horizontal(|ui| {
                        render_color_command(ui, &mut self.local_config.watchdog, "watchdog");
                    });
                    ui.end_row();
                });

            ui.add_space(20.0);
            if ui.button("Save Configuration").clicked() {
                if let Err(e) = crate::config::save_config(&self.config_path, &self.local_config) {
                    eprintln!("Failed to save config: {}", e);
                } else {
                    // Update shared config
                    *self.config.lock().unwrap() = self.local_config.clone();
                }
            }
        });

        crate::update_ui::render(
            &ctx,
            &self.update_ui_state,
            &self.flash_pause_flag,
            &self.bootloader_trigger,
            current_status.esp_connected,
        );
    }
}

fn render_color_command(ui: &mut egui::Ui, cmd: &mut ColorCommand, id_salt: &str) {
    egui::ComboBox::from_id_salt(format!("cmd_{}", id_salt))
        .selected_text(&cmd.command)
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut cmd.command, "OFF".to_string(), "OFF");
            ui.selectable_value(&mut cmd.command, "SOLID".to_string(), "SOLID");
            ui.selectable_value(&mut cmd.command, "BREATHE".to_string(), "BREATHE");
            ui.selectable_value(&mut cmd.command, "BREATHE_SLOW".to_string(), "BREATHE_SLOW");
        });

    let mut color = [cmd.r, cmd.g, cmd.b];
    ui.color_edit_button_srgb(&mut color);
    cmd.r = color[0];
    cmd.g = color[1];
    cmd.b = color[2];
}
