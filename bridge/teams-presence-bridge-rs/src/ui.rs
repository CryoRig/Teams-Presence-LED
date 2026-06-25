use eframe::egui;
use std::sync::{Arc, Mutex};
use crate::config::{Config, ColorCommand};

pub struct TeamsBridgeApp {
    config: Arc<Mutex<Config>>,
    local_config: Config,
    com_ports: Vec<String>,
    #[allow(dead_code)]
    tray_icon: tray_icon::TrayIcon,
}

impl TeamsBridgeApp {
    pub fn new(cc: &eframe::CreationContext<'_>, config: Arc<Mutex<Config>>) -> Self {
        let local_config = config.lock().unwrap().clone();
        let com_ports = serialport::available_ports()
            .map(|ports| ports.into_iter().map(|p| p.port_name).collect())
            .unwrap_or_default();
            
        let tray_menu = tray_icon::menu::Menu::new();
        let quit_i = tray_icon::menu::MenuItem::with_id("quit", "Quit", true, None);
        let _ = tray_menu.append_items(&[&quit_i]);

        let tray_icon = tray_icon::TrayIconBuilder::new()
            .with_menu(Box::new(tray_menu))
            .with_tooltip("Teams Presence Bridge")
            .with_icon(crate::create_dummy_icon())
            .build()
            .unwrap();

        let ctx = cc.egui_ctx.clone();
        std::thread::spawn(move || {
            let receiver = tray_icon::TrayIconEvent::receiver();
            while let Ok(event) = receiver.recv() {
                if let tray_icon::TrayIconEvent::Click { button: tray_icon::MouseButton::Left, button_state: tray_icon::MouseButtonState::Up, .. } = event {
                    // Move it back on screen
                    if let Some(cmd) = egui::ViewportCommand::center_on_screen(&ctx) {
                        ctx.send_viewport_cmd(cmd);
                    } else {
                        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition([100.0, 100.0].into()));
                    }
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                    ctx.request_repaint();
                }
            }
        });

        std::thread::spawn(move || {
            let receiver = tray_icon::menu::MenuEvent::receiver();
            while let Ok(event) = receiver.recv() {
                if event.id.0 == "quit" {
                    std::process::exit(0);
                }
            }
        });
            
        Self {
            config,
            local_config,
            com_ports,
            tray_icon,
        }
    }
}

impl eframe::App for TeamsBridgeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Intercept window close to hide instead of quit
        if ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            // "Hide" it by moving off-screen
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition([-10000.0, -10000.0].into()));
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Teams Presence Bridge Settings");

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.label("COM Port:");
                egui::ComboBox::from_id_source("com_port")
                    .selected_text(&self.local_config.com_port)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.local_config.com_port, "AUTO".to_string(), "AUTO");
                        for port in &self.com_ports {
                            ui.selectable_value(&mut self.local_config.com_port, port.clone(), port);
                        }
                    });
                if ui.button("Refresh").clicked() {
                    self.com_ports = serialport::available_ports()
                        .map(|ports| ports.into_iter().map(|p| p.port_name).collect())
                        .unwrap_or_default();
                }
            });

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.label("Poll Interval (ms):");
                ui.add(egui::DragValue::new(&mut self.local_config.poll_interval_ms).speed(100.0).range(100..=10000));
            });
            ui.horizontal(|ui| {
                ui.label("Ping Interval (ms):");
                ui.add(egui::DragValue::new(&mut self.local_config.ping_interval_ms).speed(1000.0).range(1000..=60000));
            });

            ui.add_space(10.0);
            ui.heading("Presence Mapping");
            
            // Sort keys to maintain stable order
            let mut keys: Vec<String> = self.local_config.presence_map.keys().cloned().collect();
            keys.sort();

            egui::ScrollArea::vertical().max_height(250.0).show(ui, |ui| {
                for key in keys {
                    ui.horizontal(|ui| {
                        ui.label(format!("{}:", key));
                        if let Some(cmd) = self.local_config.presence_map.get_mut(&key) {
                            render_color_command(ui, cmd, &key);
                        }
                    });
                }
            });

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.label("Watchdog:");
                render_color_command(ui, &mut self.local_config.watchdog, "watchdog");
            });

            ui.add_space(20.0);
            if ui.button("Save Configuration").clicked() {
                if let Err(e) = crate::config::save_config("config.json", &self.local_config) {
                    eprintln!("Failed to save config: {}", e);
                } else {
                    // Update shared config
                    *self.config.lock().unwrap() = self.local_config.clone();
                }
            }
        });
    }
}

fn render_color_command(ui: &mut egui::Ui, cmd: &mut ColorCommand, id_salt: &str) {
    egui::ComboBox::from_id_source(format!("cmd_{}", id_salt))
        .selected_text(&cmd.command)
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut cmd.command, "SOLID".to_string(), "SOLID");
            ui.selectable_value(&mut cmd.command, "BREATHE".to_string(), "BREATHE");
            ui.selectable_value(&mut cmd.command, "BLINK".to_string(), "BLINK");
        });

    let mut color = [cmd.r, cmd.g, cmd.b];
    ui.color_edit_button_srgb(&mut color);
    cmd.r = color[0];
    cmd.g = color[1];
    cmd.b = color[2];
}
