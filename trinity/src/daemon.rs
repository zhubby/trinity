//! DaemonApp — control panel root viewport that manages the system tray
//!
//! This app uses the control panel as the root viewport and serves as the daemon backbone:
//! - Creates and polls the system tray for menu events
//! - Shows the root Control Panel viewport when requested
//! - Closes the app when "Exit" is selected
//! - Spawns background translator threads (mouse listener, translation engine)
//! - Shows the Translator popup viewport when a word selection triggers translation

use eframe::App;
use egui::{Context, ViewportBuilder, ViewportCommand, ViewportId};
use log::{info, warn};
use std::sync::{Arc, Mutex, mpsc};

use crate::tray::TrayEvent;
use trinity_clipboard::{ClipboardManager, ClipboardUiAction};
use trinity_panel::{HotkeyReloadRequest, PanelApp};
use trinity_util::{
    cfg::{get_clipboard_config, get_theme, get_window_size},
    font::install_fonts,
    hotkey::{HotkeyAction, HotkeyService},
};

const WINDOW_RESIZE_HIT_ZONE: f32 = 8.0;
const CLIPBOARD_VIEWPORT_SIZE: egui::Vec2 = egui::vec2(360.0, 260.0);
const CLIPBOARD_CURSOR_OFFSET: egui::Vec2 = egui::vec2(12.0, 12.0);

fn translator_viewport_id() -> ViewportId {
    ViewportId::from_hash_of("translator_popup")
}

/// The background daemon application. Its root viewport is the control panel;
/// closing the panel hides it while keeping the tray daemon alive.
pub struct DaemonApp {
    /// Channel to receive tray events (ShowPanel / Exit)
    tray_rx: mpsc::Receiver<TrayEvent>,
    /// Whether the Control Panel viewport is currently visible
    panel_visible: bool,
    /// The PanelApp instance drawn in the root viewport
    panel_app: PanelApp,
    /// Whether the tray has been created (deferred until eframe has initialized)
    tray_created: bool,
    /// Whether global hotkeys and translation hooks have been started
    background_services_started: bool,
    /// Whether an explicit full application exit has been requested
    exit_requested: bool,
    /// Whether the About dialog is currently open
    about_visible: bool,
    /// Number of UI passes completed; first pass only paints the control panel
    ui_passes: u8,
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
    /// Whether hotkey initialization has already been attempted.
    hotkey_start_attempted: bool,
    /// Channel for daemon-side hotkey reload handling
    hotkey_reload_rx: mpsc::Receiver<HotkeyReloadRequest>,
    /// Channel for global-hotkey event handler actions.
    hotkey_event_rx: mpsc::Receiver<HotkeyAction>,
    /// Text clipboard history manager.
    clipboard_manager: ClipboardManager,
    /// Whether the clipboard picker viewport is currently visible
    clipboard_visible: bool,
    /// Last requested clipboard picker position in monitor coordinates.
    clipboard_popup_position: Option<egui::Pos2>,
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
    /// The tray is created lazily from `logic()` because macOS requires
    /// NSApp to be initialized first (eframe sets this up).
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        hotkey_service: Option<HotkeyService>,
        hotkey_event_rx: mpsc::Receiver<HotkeyAction>,
    ) -> Self {
        install_fonts(&cc.egui_ctx);

        apply_theme_preference(&cc.egui_ctx);
        cc.egui_ctx.request_repaint();

        // Channels for tray communication — dummy receiver until tray is created
        let (_, tray_rx) = mpsc::channel();

        // Channels for translation popup
        let (popup_tx, popup_rx) = mpsc::sync_channel(1);
        let (hotkey_reload_tx, hotkey_reload_rx) = mpsc::channel();
        // Shared state for background translator
        let translator_state = Arc::new(Mutex::new(TranslatorState {
            text: String::new(),
        }));

        let panel_app = PanelApp::new_from_context(&cc.egui_ctx, hotkey_reload_tx.clone());
        let clipboard_manager = ClipboardManager::new(get_clipboard_config());

        Self {
            tray_rx,
            panel_visible: true,
            panel_app,
            tray_created: false,
            background_services_started: false,
            exit_requested: false,
            about_visible: false,
            ui_passes: 0,
            translator_state,
            translator_popup_tx: popup_tx,
            translator_popup_rx: popup_rx,
            translator_popup_visible: false,
            hotkey_start_attempted: true,
            hotkey_service,
            hotkey_reload_rx,
            hotkey_event_rx,
            clipboard_manager,
            clipboard_visible: false,
            clipboard_popup_position: None,
        }
    }
}

fn apply_theme_preference(ctx: &Context) {
    let preference = match get_theme().as_str() {
        "system" => egui::ThemePreference::System,
        "light" => egui::ThemePreference::Light,
        _ => egui::ThemePreference::Dark,
    };
    ctx.set_theme(preference);
}

fn start_hotkeys() -> Option<HotkeyService> {
    info!("initializing hotkeys");
    match HotkeyService::new(&trinity_util::cfg::get_hotkey_config()) {
        Ok(service) => {
            info!("hotkeys initialized");
            Some(service)
        }
        Err(err) => {
            warn!("failed to initialize hotkeys: {err}");
            None
        }
    }
}

impl App for DaemonApp {
    fn persist_egui_memory(&self) -> bool {
        false
    }

    fn logic(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.ensure_tray_created(ctx);

        if self.ui_passes == 0 {
            let (width, height) = get_window_size();
            ctx.send_viewport_cmd(ViewportCommand::InnerSize([width, height].into()));
            ctx.send_viewport_cmd(ViewportCommand::OuterPosition([100.0, 100.0].into()));
            ctx.send_viewport_cmd(ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(ViewportCommand::Decorations(false));
            ctx.send_viewport_cmd(ViewportCommand::Resizable(true));
            ctx.send_viewport_cmd(ViewportCommand::Focus);
            ctx.request_repaint();
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        if self.process_tray_events(&ctx) {
            return;
        }

        self.show_root_panel(ui);
        if self.ui_passes == 0 {
            self.ui_passes = 1;
            ctx.request_repaint();
            return;
        }

        self.ensure_hotkeys_started();
        self.clipboard_manager.start_monitoring();
        self.process_hotkey_reload_requests();
        self.process_hotkey_actions(&ctx);

        self.process_translation_popup_events();
        self.show_translator_viewport(&ctx);
        self.show_clipboard_overlay(&ctx);

        // ── Keep daemon alive by requesting repaint ─────────────────────
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}

impl DaemonApp {
    fn ensure_tray_created(&mut self, _ctx: &Context) {
        if self.tray_created {
            return;
        }

        info!("creating system tray");
        self.tray_created = true;
        let (tray_tx, _tray_rx) = mpsc::channel();
        self.tray_rx = crate::tray::create_tray(_ctx.clone(), tray_tx);
        info!("system tray created");
    }

    fn ensure_background_services_started(&mut self, ctx: &Context) {
        if self.background_services_started {
            return;
        }

        self.background_services_started = true;
        self.ensure_hotkeys_started();
        self.clipboard_manager.start_monitoring();
        spawn_translator_threads(
            ctx.clone(),
            self.translator_state.clone(),
            self.translator_popup_tx.clone(),
        );
    }

    fn ensure_hotkeys_started(&mut self) {
        if self.hotkey_service.is_some() || self.hotkey_start_attempted {
            return;
        }

        self.hotkey_start_attempted = true;
        self.hotkey_service = start_hotkeys();
    }

    fn process_tray_events(&mut self, ctx: &Context) -> bool {
        while let Ok(event) = self.tray_rx.try_recv() {
            match event {
                TrayEvent::ShowPanel => {
                    info!("processing tray ShowPanel event");
                    self.panel_visible = true;
                    self.show_root_panel_window(ctx);
                }
                TrayEvent::Exit => {
                    info!("processing tray Exit event");
                    // Close all viewports and exit
                    self.exit_requested = true;
                    ctx.send_viewport_cmd_to(translator_viewport_id(), ViewportCommand::Close);
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                    return true;
                }
            }
        }

        false
    }

    fn process_translation_popup_events(&mut self) {
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
    }

    fn show_root_panel(&mut self, ui: &mut egui::Ui) {
        let close_requested = ui.input(|input| input.viewport().close_requested());
        if close_requested {
            if !self.exit_requested {
                self.hide_root_panel(ui.ctx());
                ui.ctx().send_viewport_cmd(ViewportCommand::CancelClose);
            }
            return;
        }

        if !self.panel_visible {
            return;
        }

        self.show_borderless_shell(ui);
        self.show_about_window(ui.ctx());
    }

    fn show_root_panel_window(&self, ctx: &Context) {
        let (width, height) = get_window_size();
        ctx.send_viewport_cmd(ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(ViewportCommand::InnerSize([width, height].into()));
        ctx.send_viewport_cmd(ViewportCommand::OuterPosition([100.0, 100.0].into()));
        ctx.send_viewport_cmd(ViewportCommand::Decorations(false));
        ctx.send_viewport_cmd(ViewportCommand::Resizable(true));
        ctx.send_viewport_cmd(ViewportCommand::Focus);
        ctx.request_repaint();
    }

    fn hide_root_panel(&mut self, ctx: &Context) {
        self.panel_visible = false;
        ctx.send_viewport_cmd(ViewportCommand::Visible(false));
        self.ensure_background_services_started(ctx);
    }

    fn exit_app(&mut self, ctx: &Context) {
        self.exit_requested = true;
        ctx.send_viewport_cmd_to(translator_viewport_id(), ViewportCommand::Close);
        ctx.send_viewport_cmd(ViewportCommand::Close);
    }

    fn show_borderless_shell(&mut self, ui: &mut egui::Ui) {
        if let Some(direction) = window_resize_direction_for_context(ui.ctx()) {
            ui.ctx().set_cursor_icon(resize_cursor_icon(direction));
            if ui.input(|input| input.pointer.button_pressed(egui::PointerButton::Primary)) {
                ui.ctx()
                    .send_viewport_cmd(ViewportCommand::BeginResize(direction));
            }
        }

        egui::Panel::top("trinity-menu-bar").show_inside(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Hide Control Panel").clicked() {
                        self.hide_root_panel(ui.ctx());
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Exit").clicked() {
                        self.exit_app(ui.ctx());
                        ui.close();
                    }
                });

                ui.menu_button("Window", |ui| {
                    if ui.button("Minimize").clicked() {
                        ui.ctx().send_viewport_cmd(ViewportCommand::Minimized(true));
                        ui.close();
                    }
                    if ui.button("Zoom").clicked() {
                        ui.ctx().send_viewport_cmd(ViewportCommand::Maximized(true));
                        ui.close();
                    }
                });

                ui.menu_button("Help", |ui| {
                    if ui.button("About Trinity").clicked() {
                        self.about_visible = true;
                        ui.close();
                    }
                });

                let row_height = ui.spacing().interact_size.y;
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), row_height),
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        let button_size = egui::vec2(row_height, row_height);
                        if ui
                            .add_sized(button_size, egui::Button::new("x"))
                            .on_hover_text("Hide Control Panel")
                            .clicked()
                        {
                            self.hide_root_panel(ui.ctx());
                        }

                        if ui
                            .add_sized(button_size, egui::Button::new("+"))
                            .on_hover_text("Zoom")
                            .clicked()
                        {
                            ui.ctx().send_viewport_cmd(ViewportCommand::Maximized(true));
                        }

                        if ui
                            .add_sized(button_size, egui::Button::new("-"))
                            .on_hover_text("Minimize")
                            .clicked()
                        {
                            ui.ctx().send_viewport_cmd(ViewportCommand::Minimized(true));
                        }

                        let drag_size = egui::vec2(ui.available_width().max(0.0), row_height);
                        if drag_size.x > 0.0 {
                            let (_rect, response) =
                                ui.allocate_exact_size(drag_size, egui::Sense::click_and_drag());
                            let pointer_pressed_on_region = response.hovered()
                                && ui.input(|input| {
                                    input.pointer.button_pressed(egui::PointerButton::Primary)
                                });
                            if pointer_pressed_on_region
                                && window_resize_direction_for_context(ui.ctx()).is_none()
                            {
                                ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
                            }
                        }
                    },
                );
            });
        });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            self.panel_app.show_inside(ui);
        });
    }

    fn show_about_window(&mut self, ctx: &Context) {
        if !self.about_visible {
            return;
        }

        let mut about_visible = self.about_visible;
        let mut close_requested = false;
        egui::Window::new("About Trinity")
            .open(&mut about_visible)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.set_min_width(320.0);
                ui.vertical_centered(|ui| {
                    ui.add_space(8.0);
                    ui.heading("Trinity");
                    ui.label("Desktop AI trifecta assistant");
                    ui.label(format!("v{}", env!("CARGO_PKG_VERSION")));
                    ui.add_space(8.0);
                    if ui.button("Close").clicked() {
                        close_requested = true;
                    }
                });
            });
        self.about_visible = about_visible && !close_requested;
    }

    fn show_translator_viewport(&mut self, ctx: &Context) {
        if !self.translator_popup_visible {
            return;
        }

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
            move |ui, _class| {
                let mut close_requested = ui.input(|input| {
                    input.viewport().close_requested() || input.key_pressed(egui::Key::Escape)
                });

                egui::CentralPanel::default().show_inside(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("x").clicked() {
                                close_requested = true;
                                ui.ctx().send_viewport_cmd_to(
                                    translator_viewport_id(),
                                    ViewportCommand::Close,
                                );
                            }
                        });
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

    fn show_clipboard_overlay(&mut self, ctx: &Context) {
        if !self.clipboard_visible {
            return;
        }

        self.clipboard_manager.reload_config(get_clipboard_config());
        let manager = self.clipboard_manager.clone();
        let mut close_requested = false;
        let position = egui::Pos2::ZERO;

        egui::Window::new("Clipboard History")
            .id(egui::Id::new("clipboard_history_overlay"))
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .fixed_pos(position)
            .fixed_size(CLIPBOARD_VIEWPORT_SIZE)
            .show(ctx, |ui| match manager.show_inside(ui) {
                ClipboardUiAction::None => {}
                ClipboardUiAction::Close => {
                    close_requested = true;
                }
                ClipboardUiAction::Paste(text) => {
                    close_requested = true;
                    std::thread::spawn(move || trinity_clipboard::paste_text(text));
                }
            });

        if close_requested {
            self.clipboard_visible = false;
        }
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
        let mut actions = self
            .hotkey_service
            .as_ref()
            .map(HotkeyService::poll_actions)
            .unwrap_or_default();
        while let Ok(action) = self.hotkey_event_rx.try_recv() {
            info!("global hotkey event handler triggered {action:?}");
            actions.push(action);
        }

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
                HotkeyAction::OpenClipboard => {
                    self.clipboard_manager.start_monitoring();
                    let position = clipboard_popup_position(ctx);
                    info!("opening clipboard overlay at {position:?}");
                    self.clipboard_popup_position = Some(position);
                    self.clipboard_visible = true;
                    self.panel_visible = true;
                    ctx.send_viewport_cmd(ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(ViewportCommand::InnerSize(CLIPBOARD_VIEWPORT_SIZE));
                    ctx.send_viewport_cmd(ViewportCommand::OuterPosition(position));
                    ctx.send_viewport_cmd(ViewportCommand::Focus);
                    ctx.request_repaint();
                }
                HotkeyAction::QuitApp => {
                    self.exit_app(ctx);
                }
            }
        }
    }
}

fn window_resize_direction_for_context(ctx: &Context) -> Option<egui::viewport::ResizeDirection> {
    let (viewport, content_rect, pointer_pos) = ctx.input(|input| {
        (
            input.viewport().clone(),
            input.content_rect(),
            input.pointer.latest_pos(),
        )
    });
    if viewport.fullscreen == Some(true) || viewport.maximized == Some(true) {
        return None;
    }
    window_resize_direction(pointer_pos?, content_rect, WINDOW_RESIZE_HIT_ZONE)
}

fn window_resize_direction(
    pos: egui::Pos2,
    rect: egui::Rect,
    hit_zone: f32,
) -> Option<egui::viewport::ResizeDirection> {
    use egui::viewport::ResizeDirection;

    if !rect.contains(pos) {
        return None;
    }

    let near_left = pos.x <= rect.left() + hit_zone;
    let near_right = pos.x >= rect.right() - hit_zone;
    let near_top = pos.y <= rect.top() + hit_zone;
    let near_bottom = pos.y >= rect.bottom() - hit_zone;

    match (near_left, near_right, near_top, near_bottom) {
        (true, _, true, _) => Some(ResizeDirection::NorthWest),
        (_, true, true, _) => Some(ResizeDirection::NorthEast),
        (true, _, _, true) => Some(ResizeDirection::SouthWest),
        (_, true, _, true) => Some(ResizeDirection::SouthEast),
        (true, _, _, _) => Some(ResizeDirection::West),
        (_, true, _, _) => Some(ResizeDirection::East),
        (_, _, true, _) => Some(ResizeDirection::North),
        (_, _, _, true) => Some(ResizeDirection::South),
        _ => None,
    }
}

fn resize_cursor_icon(direction: egui::viewport::ResizeDirection) -> egui::CursorIcon {
    use egui::{CursorIcon, viewport::ResizeDirection};

    match direction {
        ResizeDirection::North => CursorIcon::ResizeNorth,
        ResizeDirection::South => CursorIcon::ResizeSouth,
        ResizeDirection::East => CursorIcon::ResizeEast,
        ResizeDirection::West => CursorIcon::ResizeWest,
        ResizeDirection::NorthEast => CursorIcon::ResizeNorthEast,
        ResizeDirection::SouthEast => CursorIcon::ResizeSouthEast,
        ResizeDirection::NorthWest => CursorIcon::ResizeNorthWest,
        ResizeDirection::SouthWest => CursorIcon::ResizeSouthWest,
    }
}

fn clipboard_popup_position(ctx: &Context) -> egui::Pos2 {
    let cursor = current_mouse_position()
        .or_else(|| cursor_position_from_egui(ctx))
        .unwrap_or(egui::pos2(100.0, 100.0));
    let monitor_size = ctx
        .input(|input| input.viewport().monitor_size)
        .or_else(display_size_points);

    clamp_popup_position(cursor + CLIPBOARD_CURSOR_OFFSET, monitor_size)
}

fn cursor_position_from_egui(ctx: &Context) -> Option<egui::Pos2> {
    ctx.input(|input| {
        let pointer_pos = input.pointer.latest_pos()?;
        let viewport_rect = input
            .viewport()
            .outer_rect
            .or(input.viewport().inner_rect)?;
        Some(viewport_rect.min + pointer_pos.to_vec2())
    })
}

fn clamp_popup_position(pos: egui::Pos2, monitor_size: Option<egui::Vec2>) -> egui::Pos2 {
    let Some(monitor_size) = monitor_size else {
        return pos;
    };

    let max_x = (monitor_size.x - CLIPBOARD_VIEWPORT_SIZE.x - 8.0).max(0.0);
    let max_y = (monitor_size.y - CLIPBOARD_VIEWPORT_SIZE.y - 8.0).max(0.0);
    egui::pos2(pos.x.clamp(0.0, max_x), pos.y.clamp(0.0, max_y))
}

fn display_size_points() -> Option<egui::Vec2> {
    let (width, height) = rdev::display_size().ok()?;
    Some(egui::vec2(width as f32, height as f32))
}

#[cfg(target_os = "macos")]
fn current_mouse_position() -> Option<egui::Pos2> {
    use objc::{class, msg_send, sel, sel_impl};

    #[repr(C)]
    struct AppKitPoint {
        x: f64,
        y: f64,
    }

    let point: AppKitPoint = unsafe { msg_send![class!(NSEvent), mouseLocation] };
    let (_, display_height) = rdev::display_size().ok()?;
    Some(egui::pos2(
        point.x as f32,
        display_height as f32 - point.y as f32,
    ))
}

#[cfg(target_os = "windows")]
fn current_mouse_position() -> Option<egui::Pos2> {
    use winapi::{shared::windef::POINT, um::winuser::GetCursorPos};

    let mut point = POINT { x: 0, y: 0 };
    let ok = unsafe { GetCursorPos(&mut point) };
    (ok != 0).then_some(egui::pos2(point.x as f32, point.y as f32))
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn current_mouse_position() -> Option<egui::Pos2> {
    None
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
    #[cfg(target_os = "macos")]
    {
        let _ = (ctx, state, popup_tx);
        warn!(
            "automatic selection translation is disabled on macOS because rdev::listen crashes on modifier key events"
        );
    }

    #[cfg(not(target_os = "macos"))]
    spawn_translator_threads_with_rdev(ctx, state, popup_tx);
}

#[cfg(not(target_os = "macos"))]
fn spawn_translator_threads_with_rdev(
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
