//! Trinity Panel - Settings and control panel GUI
//!
//! Provides a settings panel for the Trinity application where users can:
//! - Configure translation API settings
//! - Configure hotkeys
//! - Switch themes (dark/light)
//! - Manage clipboard preferences
//! - Manage dictation preferences
//!
//! This module is currently a stub. Full implementation is planned.

use std::sync::mpsc;

use egui::{Context, RichText};
use trinity_util::{
    cfg::{SETTINGS, get_theme, save_hotkey_config},
    hotkey::HotkeyConfig,
};

pub struct HotkeyReloadRequest {
    pub config: HotkeyConfig,
    pub result_tx: mpsc::Sender<Result<(), String>>,
}

pub type HotkeyReloadTx = mpsc::Sender<HotkeyReloadRequest>;

/// Initialize the panel module
pub fn init() {
    // TODO: implement panel initialization
}

/// Settings panel application
pub struct PanelApp {
    /// Current API URL (editable)
    api_url: String,
    /// Current theme selection
    theme: String,
    hotkey_config: HotkeyConfig,
    hotkey_status: Option<Result<String, String>>,
    hotkey_reload_tx: Option<HotkeyReloadTx>,
    hotkey_result_rx: Option<mpsc::Receiver<Result<(), String>>>,
    pending_hotkey_config: Option<HotkeyConfig>,
}

impl PanelApp {
    /// Create a new PanelApp from an egui Context (for daemon viewport usage).
    ///
    /// Unlike `new(cc: &CreationContext)` which is for standalone eframe apps,
    /// this constructor uses the daemon's existing context.
    pub fn new_from_context(ctx: &Context, hotkey_reload_tx: HotkeyReloadTx) -> Self {
        // Read current settings, then release the lock before loading hotkeys.
        let (api_url, theme) = {
            let settings = SETTINGS.lock().unwrap_or_else(|e| e.into_inner());
            let api_url = settings
                .get_string("api")
                .unwrap_or_else(|_| "https://deepl.zu1k.com/translate".to_string());
            let theme = settings
                .get_string("window.theme")
                .unwrap_or_else(|_| "dark".to_string());
            (api_url, theme)
        };

        // Apply theme
        match theme.as_str() {
            "light" => ctx.set_visuals(egui::Visuals::light()),
            _ => ctx.set_visuals(egui::Visuals::dark()),
        }

        let hotkey_config = trinity_util::cfg::get_hotkey_config();

        Self {
            api_url,
            theme,
            hotkey_config,
            hotkey_status: None,
            hotkey_reload_tx: Some(hotkey_reload_tx),
            hotkey_result_rx: None,
            pending_hotkey_config: None,
        }
    }

    /// Create a new panel app with the given creation context (for standalone use)
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Apply theme from config
        match get_theme().as_str() {
            "light" => cc.egui_ctx.set_visuals(egui::Visuals::light()),
            _ => cc.egui_ctx.set_visuals(egui::Visuals::dark()),
        }

        let (api_url, theme) = {
            let settings = SETTINGS.lock().unwrap_or_else(|e| e.into_inner());
            let api_url = settings
                .get_string("api")
                .unwrap_or_else(|_| "https://deepl.zu1k.com/translate".to_string());
            let theme = settings
                .get_string("window.theme")
                .unwrap_or_else(|_| "dark".to_string());
            (api_url, theme)
        };

        let hotkey_config = trinity_util::cfg::get_hotkey_config();

        Self {
            api_url,
            theme,
            hotkey_config,
            hotkey_status: None,
            hotkey_reload_tx: None,
            hotkey_result_rx: None,
            pending_hotkey_config: None,
        }
    }

    /// Draw the panel contents inside an existing egui Ui.
    ///
    /// This is used by DaemonApp's `show_viewport_immediate` callback
    /// since the panel is drawn as an immediate-mode viewport, not
    /// as a standalone eframe::App.
    pub fn show_inside(&mut self, ui: &mut egui::Ui) {
        self.poll_hotkey_reload_result();

        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.heading("Trinity Settings");
            ui.separator();

            // ── API Configuration ─────────────────────────────────────
            ui.group(|ui| {
                ui.label(RichText::new("Translation API").strong());
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label("API URL:");
                    ui.add(egui::TextEdit::singleline(&mut self.api_url).desired_width(300.0));
                });
                ui.horizontal(|ui| {
                    if ui.button("Save API URL").clicked() {
                        // TODO: persist config change
                        log::info!("API URL save requested: {}", self.api_url);
                    }
                    ui.label(
                        RichText::new("(changes not persisted yet)")
                            .small()
                            .color(egui::Color32::GRAY),
                    );
                });
            });

            ui.add_space(8.0);

            // ── Theme ──────────────────────────────────────────────────
            ui.group(|ui| {
                ui.label(RichText::new("Theme").strong());
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label("Color scheme:");
                    egui::ComboBox::from_id_salt("theme_combo")
                        .selected_text(&self.theme)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.theme, "dark".to_string(), "Dark");
                            ui.selectable_value(&mut self.theme, "light".to_string(), "Light");
                        });
                });
            });

            ui.add_space(8.0);

            // ── Hotkey Configuration ────────────────────────────────────
            ui.group(|ui| {
                ui.label(RichText::new("Hotkeys").strong());
                ui.add_space(4.0);
                ui.vertical(|ui| {
                    hotkey_row(
                        ui,
                        "Open translator:",
                        &mut self.hotkey_config.open_translator,
                    );
                    hotkey_row(
                        ui,
                        "Translate selection:",
                        &mut self.hotkey_config.translate_selection,
                    );
                    hotkey_row(ui, "Quit app:", &mut self.hotkey_config.quit_app);
                    ui.add_space(4.0);

                    ui.horizontal(|ui| {
                        if ui.button("Save Hotkeys").clicked() {
                            self.save_hotkeys();
                        }
                        if ui.button("Reset Defaults").clicked() {
                            self.hotkey_config = HotkeyConfig::default();
                        }
                    });

                    if let Some(status) = &self.hotkey_status {
                        match status {
                            Ok(message) => {
                                ui.label(
                                    RichText::new(message).small().color(egui::Color32::GREEN),
                                );
                            }
                            Err(message) => {
                                ui.label(RichText::new(message).small().color(egui::Color32::RED));
                            }
                        }
                    }
                });
            });

            ui.add_space(8.0);

            // ── About ──────────────────────────────────────────────────
            ui.group(|ui| {
                ui.label(RichText::new("About").strong());
                ui.add_space(4.0);
                ui.label("Trinity v0.5.0");
                ui.label("Desktop AI trifecta assistant");
                ui.hyperlink_to("GitHub", "https://github.com/zu1k/translator");
            });
        });
    }
}

impl PanelApp {
    fn save_hotkeys(&mut self) {
        if let Err(err) = self.hotkey_config.validate() {
            self.hotkey_status = Some(Err(err.to_string()));
            return;
        }

        let Some(hotkey_reload_tx) = &self.hotkey_reload_tx else {
            if let Err(err) = save_hotkey_config(&self.hotkey_config) {
                self.hotkey_status = Some(Err(format!("failed to save hotkeys: {err}")));
                return;
            }
            self.hotkey_status = Some(Ok("Hotkeys saved.".to_string()));
            return;
        };

        let (result_tx, result_rx) = mpsc::channel();
        let config = self.hotkey_config.clone();
        let request = HotkeyReloadRequest {
            config: config.clone(),
            result_tx,
        };

        if hotkey_reload_tx.send(request).is_err() {
            self.hotkey_status = Some(Err("failed to reload hotkeys in daemon".to_string()));
            return;
        }

        self.hotkey_result_rx = Some(result_rx);
        self.pending_hotkey_config = Some(config);
        self.hotkey_status = Some(Ok("Reloading hotkeys...".to_string()));
    }

    fn poll_hotkey_reload_result(&mut self) {
        let Some(result_rx) = &self.hotkey_result_rx else {
            return;
        };

        match result_rx.try_recv() {
            Ok(Ok(())) => {
                let Some(config) = self.pending_hotkey_config.take() else {
                    self.hotkey_status = Some(Ok("Hotkeys active.".to_string()));
                    self.hotkey_result_rx = None;
                    return;
                };
                self.hotkey_status = match save_hotkey_config(&config) {
                    Ok(()) => Some(Ok("Hotkeys saved and active.".to_string())),
                    Err(err) => Some(Err(format!("hotkeys active but failed to save: {err}"))),
                };
                self.hotkey_result_rx = None;
            }
            Ok(Err(message)) => {
                self.pending_hotkey_config = None;
                self.hotkey_status = Some(Err(message));
                self.hotkey_result_rx = None;
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.pending_hotkey_config = None;
                self.hotkey_status = Some(Err("hotkey reload status channel closed".to_string()));
                self.hotkey_result_rx = None;
            }
        }
    }
}

fn hotkey_row(ui: &mut egui::Ui, label: &str, value: &mut String) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::TextEdit::singleline(value).desired_width(220.0));
    });
}

impl eframe::App for PanelApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.show_inside(ui);
    }
}
