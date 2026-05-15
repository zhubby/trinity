//! DaemonApp — invisible background eframe app that manages the system tray
//!
//! This app has a hidden main viewport and serves as the daemon backbone:
//! - Creates and polls the system tray for menu events
//! - Shows the Settings Panel viewport when requested
//! - Closes the app when "Exit" is selected
//! - Spawns background translator threads (mouse listener, translation engine)
//! - Shows the Translator popup viewport when a word selection triggers translation

use eframe::App;
use egui::{Context, ViewportBuilder, ViewportCommand, ViewportId};
use log::warn;
use std::sync::{Arc, Mutex, mpsc};

use crate::tray::TrayEvent;
use trinity_panel::{HotkeyReloadRequest, PanelApp};
use trinity_util::{
    cfg::{get_theme, get_window_size},
    font::install_fonts,
    hotkey::{HotkeyAction, HotkeyService},
};

fn panel_viewport_id() -> ViewportId {
    ViewportId::from_hash_of("panel")
}

fn translator_viewport_id() -> ViewportId {
    ViewportId::from_hash_of("translator_popup")
}

/// The background daemon application. Its main viewport is invisible;
/// all user-facing windows are spawned as secondary viewports.
pub struct DaemonApp {
    /// Channel to receive tray events (ShowPanel / Exit)
    tray_rx: mpsc::Receiver<TrayEvent>,
    /// Whether the Settings Panel viewport is currently visible
    panel_visible: bool,
    /// The PanelApp instance that draws the settings panel viewport
    panel_app: Option<PanelApp>,
    /// Whether the tray has been created (deferred to first ui() call on macOS)
    tray_created: bool,
    /// Shared translation state for background translator threads
    translator_state: Arc<Mutex<TranslatorState>>,
    /// Channel to signal a new translation popup should appear
    translator_popup_tx: mpsc::SyncSender<TranslationPopupEvent>,
    /// Channel to receive signals that a translation popup should appear
    translator_popup_rx: mpsc::Receiver<TranslationPopupEvent>,
    /// Whether the translator popup viewport is currently visible
    translator_popup_visible: bool,
    /// Application-wide system hotkey service
    hotkey_service: Option<HotkeyService>,
    /// Channel for panel-triggered hotkey reload requests
    hotkey_reload_tx: mpsc::Sender<HotkeyReloadRequest>,
    /// Channel for daemon-side hotkey reload handling
    hotkey_reload_rx: mpsc::Receiver<HotkeyReloadRequest>,
}

/// Translation popup event carrying the text to display
struct TranslationPopupEvent {
    text: String,
}

/// Background translation state shared between threads
struct TranslatorState {
    /// Current translation text (shown in popup)
    text: String,
}

impl DaemonApp {
    /// Create a new DaemonApp.
    ///
    /// The tray is created lazily on the first `ui()` call because macOS
    /// requires NSApp to be initialized first (eframe sets this up).
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        install_fonts(&cc.egui_ctx);

        // Apply theme
        match get_theme().as_str() {
            "light" => cc.egui_ctx.set_visuals(egui::Visuals::light()),
            _ => cc.egui_ctx.set_visuals(egui::Visuals::dark()),
        }

        // Channels for tray communication — dummy receiver until tray is created
        let (_, tray_rx) = mpsc::channel();

        // Channels for translation popup
        let (popup_tx, popup_rx) = mpsc::sync_channel(1);
        let (hotkey_reload_tx, hotkey_reload_rx) = mpsc::channel();
        let hotkey_service = match HotkeyService::new(&trinity_util::cfg::get_hotkey_config()) {
            Ok(service) => Some(service),
            Err(err) => {
                warn!("failed to initialize hotkeys: {err}");
                None
            }
        };

        // Shared state for background translator
        let translator_state = Arc::new(Mutex::new(TranslatorState {
            text: String::new(),
        }));

        Self {
            tray_rx,
            panel_visible: false,
            panel_app: None,
            tray_created: false,
            translator_state,
            translator_popup_tx: popup_tx,
            translator_popup_rx: popup_rx,
            translator_popup_visible: false,
            hotkey_service,
            hotkey_reload_tx,
            hotkey_reload_rx,
        }
    }
}

impl App for DaemonApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.process_hotkey_reload_requests();
        self.process_hotkey_actions(&ctx);

        // ── Deferred tray creation (first frame) ───────────────────────
        if !self.tray_created {
            self.tray_created = true;
            let (tray_tx, _tray_rx) = mpsc::channel();
            self.tray_rx = crate::tray::create_tray(tray_tx);

            // ── Spawn background translator threads ────────────────────
            spawn_translator_threads(
                ctx.clone(),
                self.translator_state.clone(),
                self.translator_popup_tx.clone(),
            );
        }

        // ── Process tray events ─────────────────────────────────────────
        while let Ok(event) = self.tray_rx.try_recv() {
            match event {
                TrayEvent::ShowPanel => {
                    self.panel_visible = true;
                    if self.panel_app.is_none() {
                        // We create a PanelApp context lazily. Since PanelApp::new()
                        // needs a CreationContext, we'll create a simple panel
                        // that uses the daemon's context directly.
                        self.panel_app = Some(PanelApp::new_from_context(
                            &ctx,
                            self.hotkey_reload_tx.clone(),
                        ));
                    }
                }
                TrayEvent::Exit => {
                    // Close all viewports and exit
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                    return;
                }
            }
        }

        // ── Show Settings Panel viewport ────────────────────────────────
        if self.panel_visible
            && let Some(panel_app) = &mut self.panel_app
        {
            let (width, height) = get_window_size();
            let close_requested = ctx.show_viewport_immediate(
                panel_viewport_id(),
                ViewportBuilder::default()
                    .with_title("Trinity Settings")
                    .with_inner_size([width, height])
                    .with_resizable(true),
                |ui, _class| {
                    let close_requested = ui.input(|input| input.viewport().close_requested());
                    // Draw the panel content using PanelApp's ui logic
                    panel_app.show_inside(ui);
                    close_requested
                },
            );
            if close_requested {
                self.panel_visible = false;
                self.panel_app = None;
            }
        }

        // ── Process translation popup events ────────────────────────────
        while let Ok(popup) = self.translator_popup_rx.try_recv() {
            self.translator_popup_visible = true;
            {
                let mut state = self
                    .translator_state
                    .lock()
                    .unwrap_or_else(|err| err.into_inner());
                state.text = popup.text;
            }
        }

        if self.translator_popup_visible {
            let (width, height) = get_window_size();
            let text = self
                .translator_state
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .text
                .clone();
            let text = if text.trim().is_empty() {
                "请选中需要翻译的文字触发划词翻译".to_string()
            } else {
                text
            };
            let close_requested = ctx.show_viewport_immediate(
                translator_viewport_id(),
                ViewportBuilder::default()
                    .with_title("Translator")
                    .with_always_on_top()
                    .with_decorations(false)
                    .with_inner_size([width, height]),
                |ui, _class| {
                    let mut close_requested = ui.input(|input| {
                        input.viewport().close_requested() || input.key_pressed(egui::Key::Escape)
                    });
                    egui::CentralPanel::default().show_inside(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.button("x").clicked() {
                                        close_requested = true;
                                        ui.ctx().send_viewport_cmd_to(
                                            translator_viewport_id(),
                                            ViewportCommand::Close,
                                        );
                                    }
                                },
                            );
                        });
                        ui.label(&text);
                    });
                    close_requested
                },
            );
            if close_requested {
                self.translator_popup_visible = false;
            }
        }

        // ── Keep daemon alive by requesting repaint ─────────────────────
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}

impl DaemonApp {
    fn process_hotkey_reload_requests(&mut self) {
        while let Ok(request) = self.hotkey_reload_rx.try_recv() {
            let result = match &mut self.hotkey_service {
                Some(service) => service.reload(&request.config),
                None => HotkeyService::new(&request.config).map(|service| {
                    self.hotkey_service = Some(service);
                }),
            };

            match result {
                Ok(()) => {
                    let _ = request.result_tx.send(Ok(()));
                }
                Err(err) => {
                    let message = err.to_string();
                    let _ = request.result_tx.send(Err(message));
                }
            }
        }
    }

    fn process_hotkey_actions(&mut self, ctx: &Context) {
        let actions = self
            .hotkey_service
            .as_ref()
            .map(HotkeyService::poll_actions)
            .unwrap_or_default();

        for action in actions {
            match action {
                HotkeyAction::OpenTranslator => {
                    self.translator_popup_visible = true;
                    ctx.request_repaint();
                }
                HotkeyAction::TranslateSelection => trigger_translate_selection(
                    ctx.clone(),
                    self.translator_state.clone(),
                    self.translator_popup_tx.clone(),
                ),
                HotkeyAction::QuitApp => {
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                }
            }
        }
    }
}

/// Spawn the background translator threads:
/// 1. Mouse listener (rdev::listen) — detects word selection
/// 2. Translation poller — waits for selection, runs DeepL translation,
///    then signals popup via channel
fn spawn_translator_threads(
    ctx: Context,
    state: Arc<Mutex<TranslatorState>>,
    popup_tx: mpsc::SyncSender<TranslationPopupEvent>,
) {
    use std::{
        thread::{self, sleep},
        time::Duration,
    };

    // ── Mouse listener thread ───────────────────────────────────
    {
        let mouse_state = Arc::new(Mutex::new(trinity_translator::MouseState::new()));
        let mouse_state_clone = mouse_state.clone();

        thread::spawn(move || {
            if let Err(err) = rdev::listen(move |event| {
                let mut ms = mouse_state_clone.lock().unwrap_or_else(|e| e.into_inner());
                match event.event_type {
                    rdev::EventType::ButtonPress(rdev::Button::Left) => ms.down(),
                    rdev::EventType::ButtonRelease(rdev::Button::Left) => ms.release(),
                    rdev::EventType::MouseMove { .. } => ms.moving(),
                    _ => {}
                }
            }) {
                warn!("rdev listen error: {:?}", err);
            }
        });

        // ── Translation poller thread ────────────────────────────
        thread::spawn(move || {
            let mut clipboard_last = String::new();
            loop {
                let is_select = {
                    let mut ms = mouse_state.lock().unwrap_or_else(|e| e.into_inner());
                    ms.is_select()
                };

                if is_select
                    && !ctx.input(|i| i.pointer.has_pointer())
                    && let Some(text_new) = trinity_translator::hotkey::ctrl_c()
                    && text_new != clipboard_last
                {
                    clipboard_last = text_new.clone();

                    // Store text in shared state
                    {
                        let mut s = state.lock().unwrap_or_else(|e| e.into_inner());
                        s.text = text_new.clone();
                    }

                    // Translate
                    let (source_lang, target_lang) = {
                        // Default languages; in future read from shared settings
                        (deepl::Lang::Auto, deepl::Lang::ZH)
                    };
                    let result = deepl::translate(
                        &trinity_util::cfg::get_api(),
                        text_new,
                        target_lang,
                        source_lang,
                    )
                    .unwrap_or_else(|_| "翻译接口失效，请更换".to_string());

                    // Update shared state and signal popup
                    {
                        let mut s = state.lock().unwrap_or_else(|e| e.into_inner());
                        s.text = result.clone();
                    }
                    popup_tx.send(TranslationPopupEvent { text: result }).ok();
                    ctx.request_repaint();
                }

                sleep(Duration::from_millis(100));
            }
        });
    }
}

fn trigger_translate_selection(
    ctx: Context,
    state: Arc<Mutex<TranslatorState>>,
    popup_tx: mpsc::SyncSender<TranslationPopupEvent>,
) {
    std::thread::spawn(move || {
        let Some(text_new) = trinity_translator::hotkey::ctrl_c() else {
            return;
        };
        let text_new = text_new.trim().to_string();
        if text_new.is_empty() {
            return;
        }

        {
            let mut s = state.lock().unwrap_or_else(|err| err.into_inner());
            s.text = text_new.clone();
        }

        let result = deepl::translate(
            &trinity_util::cfg::get_api(),
            text_new,
            deepl::Lang::ZH,
            deepl::Lang::Auto,
        )
        .unwrap_or_else(|_| "翻译接口失效，请更换".to_string());

        {
            let mut s = state.lock().unwrap_or_else(|err| err.into_inner());
            s.text = result.clone();
        }
        popup_tx.send(TranslationPopupEvent { text: result }).ok();
        ctx.request_repaint();
    });
}
