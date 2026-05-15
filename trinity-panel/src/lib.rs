//! Trinity Panel - control panel GUI
//!
//! Provides a control panel for the Trinity application where users can:
//! - Configure general application preferences
//! - Configure hotkeys
//! - Manage clipboard preferences
//! - Configure translation service settings
//! - Manage voice service preferences

use std::sync::mpsc;

use egui::{Context, RichText};
use egui_dock::{DockArea, DockState, TabViewer};
use egui_theme_switch::ThemeSwitch;
use trinity_util::{
    ClipboardConfig,
    cfg::{
        get_api, get_clipboard_config, get_theme, save_basic_config, save_clipboard_config,
        save_hotkey_config, settings_path,
    },
    font::install_fonts,
    hotkey::HotkeyConfig,
};

const DEFAULT_API_URL: &str = "https://deepl.zu1k.com/translate";

pub struct HotkeyReloadRequest {
    pub config: HotkeyConfig,
    pub result_tx: mpsc::Sender<Result<(), String>>,
}

pub type HotkeyReloadTx = mpsc::Sender<HotkeyReloadRequest>;

/// Initialize the panel module
pub fn init() {
    // TODO: implement panel initialization
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ControlPanelTab {
    General,
    Hotkeys,
    Clipboard,
    TranslationService,
    VoiceService,
}

impl ControlPanelTab {
    const ALL: [Self; 5] = [
        Self::General,
        Self::Hotkeys,
        Self::Clipboard,
        Self::TranslationService,
        Self::VoiceService,
    ];

    #[must_use]
    fn title(self) -> &'static str {
        match self {
            Self::General => "通用",
            Self::Hotkeys => "快捷键",
            Self::Clipboard => "剪切板",
            Self::TranslationService => "翻译服务",
            Self::VoiceService => "语音服务",
        }
    }
}

/// Control panel application.
pub struct PanelApp {
    /// Current API URL (editable)
    api_url: String,
    /// Current theme selection
    theme: String,
    dock_state: DockState<ControlPanelTab>,
    basic_status: Option<Result<String, String>>,
    hotkey_config: HotkeyConfig,
    hotkey_status: Option<Result<String, String>>,
    hotkey_reload_tx: Option<HotkeyReloadTx>,
    hotkey_result_rx: Option<mpsc::Receiver<Result<(), String>>>,
    pending_hotkey_config: Option<HotkeyConfig>,
    clipboard_config: ClipboardConfig,
    clipboard_status: Option<Result<String, String>>,
}

impl PanelApp {
    /// Create a new PanelApp from an egui Context (for daemon viewport usage).
    ///
    /// Unlike `new(cc: &CreationContext)` which is for standalone eframe apps,
    /// this constructor uses the daemon's existing context.
    pub fn new_from_context(ctx: &Context, hotkey_reload_tx: HotkeyReloadTx) -> Self {
        let (api_url, theme) = load_basic_config();
        apply_theme(ctx, &theme);

        Self::from_config(api_url, theme, Some(hotkey_reload_tx))
    }

    /// Create a new panel app with the given creation context (for standalone use).
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let (api_url, theme) = load_basic_config();
        apply_theme(&cc.egui_ctx, &theme);

        Self::from_config(api_url, theme, None)
    }

    /// Draw the panel contents inside an existing egui Ui.
    ///
    /// This is used by DaemonApp's root viewport, where the panel is drawn
    /// inside an existing immediate-mode surface.
    pub fn show_inside(&mut self, ui: &mut egui::Ui) {
        self.poll_hotkey_reload_result();

        egui::CentralPanel::default().show_inside(ui, |ui| {
            self.show_header(ui);
            ui.add_space(8.0);

            let mut dock_state =
                std::mem::replace(&mut self.dock_state, Self::default_dock_state());
            let mut viewer = PanelTabViewer { app: self };
            DockArea::new(&mut dock_state)
                .show_add_buttons(false)
                .show_add_popup(false)
                .show_close_buttons(false)
                .draggable_tabs(false)
                .tab_context_menus(false)
                .show_leaf_close_all_buttons(false)
                .show_leaf_collapse_buttons(false)
                .show_inside(ui, &mut viewer);
            self.dock_state = dock_state;
        });
    }

    fn from_config(
        api_url: String,
        theme: String,
        hotkey_reload_tx: Option<HotkeyReloadTx>,
    ) -> Self {
        Self {
            api_url,
            theme,
            dock_state: Self::default_dock_state(),
            basic_status: None,
            hotkey_config: trinity_util::cfg::get_hotkey_config(),
            hotkey_status: None,
            hotkey_reload_tx,
            hotkey_result_rx: None,
            pending_hotkey_config: None,
            clipboard_config: get_clipboard_config(),
            clipboard_status: None,
        }
    }

    fn default_dock_state() -> DockState<ControlPanelTab> {
        DockState::new(ControlPanelTab::ALL.to_vec())
    }

    fn show_header(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Trinity Control Panel");
            ui.add_space(8.0);
            ui.label(
                RichText::new("v0.5.0")
                    .small()
                    .color(ui.visuals().weak_text_color()),
            );
        });
    }

    fn show_general_tab(&mut self, ui: &mut egui::Ui) {
        section_title(ui, "外观");
        ui.horizontal(|ui| {
            ui.label("主题");
            let mut preference = theme_preference(&self.theme);
            if ui.add(ThemeSwitch::new(&mut preference)).changed() {
                self.apply_and_save_theme(ui.ctx(), preference);
            }
            ui.label(RichText::new(theme_preference_label(preference)).small());
        });

        ui.add_space(12.0);
        section_title(ui, "配置");
        ui.horizontal_wrapped(|ui| {
            ui.label("配置文件");
            ui.monospace(settings_path().display().to_string());
        });

        ui.add_space(12.0);
        self.show_basic_actions(ui);
    }

    fn show_hotkeys_tab(&mut self, ui: &mut egui::Ui) {
        section_title(ui, "全局快捷键");
        hotkey_row(ui, "打开翻译窗口", &mut self.hotkey_config.open_translator);
        hotkey_row(
            ui,
            "翻译当前选区",
            &mut self.hotkey_config.translate_selection,
        );
        hotkey_row(ui, "打开剪切板", &mut self.hotkey_config.open_clipboard);
        hotkey_row(ui, "退出 Trinity", &mut self.hotkey_config.quit_app);

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("保存快捷键").clicked() {
                self.save_hotkeys();
            }
            if ui.button("恢复默认").clicked() {
                self.hotkey_config = HotkeyConfig::default();
                self.hotkey_status = None;
            }
        });

        show_status(ui, &self.hotkey_status);
    }

    fn show_clipboard_tab(&mut self, ui: &mut egui::Ui) {
        section_title(ui, "剪切板");
        ui.horizontal(|ui| {
            ui.add_sized([120.0, 20.0], egui::Label::new("存储数量"));
            ui.add(
                egui::DragValue::new(&mut self.clipboard_config.capacity)
                    .range(ClipboardConfig::MIN_CAPACITY..=ClipboardConfig::MAX_CAPACITY)
                    .speed(1),
            );
        });
        ui.horizontal(|ui| {
            ui.add_sized([120.0, 20.0], egui::Label::new("面板显示条数"));
            ui.add(
                egui::DragValue::new(&mut self.clipboard_config.panel_page_size)
                    .range(
                        ClipboardConfig::MIN_PANEL_PAGE_SIZE..=ClipboardConfig::MAX_PANEL_PAGE_SIZE,
                    )
                    .speed(1),
            );
        });
        hotkey_row(ui, "唤起快捷键", &mut self.hotkey_config.open_clipboard);

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("保存剪切板设置").clicked() {
                self.save_clipboard_settings();
            }
            if ui.button("恢复默认").clicked() {
                self.clipboard_config = ClipboardConfig::default();
                self.hotkey_config.open_clipboard =
                    HotkeyConfig::DEFAULT_OPEN_CLIPBOARD.to_string();
                self.clipboard_status = None;
            }
        });

        show_status(ui, &self.clipboard_status);
    }

    fn show_translation_tab(&mut self, ui: &mut egui::Ui) {
        section_title(ui, "DeepL 翻译服务");
        ui.label("API URL");
        ui.add(
            egui::TextEdit::singleline(&mut self.api_url)
                .desired_width(f32::INFINITY)
                .hint_text(DEFAULT_API_URL),
        );

        ui.add_space(8.0);
        self.show_basic_actions(ui);
    }

    fn show_voice_tab(ui: &mut egui::Ui) {
        section_title(ui, "语音服务");
        readonly_row(ui, "模块状态", "待实现");
        readonly_row(ui, "配置写入", "暂未开放");
    }

    fn show_basic_actions(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("保存基础设置").clicked() {
                self.save_basic_settings(ui.ctx());
            }
        });
        show_status(ui, &self.basic_status);
    }

    fn save_basic_settings(&mut self, ctx: &Context) {
        let theme = normalize_theme(&self.theme);
        self.theme = theme.to_string();

        match save_basic_config(self.api_url.trim(), theme) {
            Ok(()) => {
                apply_theme(ctx, theme);
                self.basic_status = Some(Ok("基础设置已保存。".to_string()));
            }
            Err(err) => {
                self.basic_status = Some(Err(format!("保存基础设置失败: {err}")));
            }
        }
    }

    fn apply_and_save_theme(&mut self, ctx: &Context, preference: egui::ThemePreference) {
        let theme = theme_from_preference(preference);
        self.theme = theme.to_string();
        ctx.set_theme(preference);
        install_fonts(ctx);

        self.basic_status = match save_basic_config(self.api_url.trim(), theme) {
            Ok(()) => Some(Ok("主题已保存。".to_string())),
            Err(err) => Some(Err(format!("保存主题失败: {err}"))),
        };
    }

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

    fn save_clipboard_settings(&mut self) {
        self.clipboard_config = self.clipboard_config.normalized();
        if let Err(err) = self.hotkey_config.validate() {
            self.clipboard_status = Some(Err(err.to_string()));
            return;
        }

        match save_clipboard_config(self.clipboard_config) {
            Ok(()) => {
                self.clipboard_status = Some(Ok("剪切板设置已保存。".to_string()));
                self.save_hotkeys();
            }
            Err(err) => {
                self.clipboard_status = Some(Err(format!("保存剪切板设置失败: {err}")));
            }
        }
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

struct PanelTabViewer<'a> {
    app: &'a mut PanelApp,
}

impl TabViewer for PanelTabViewer<'_> {
    type Tab = ControlPanelTab;

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
                match tab {
                    ControlPanelTab::General => self.app.show_general_tab(ui),
                    ControlPanelTab::Hotkeys => self.app.show_hotkeys_tab(ui),
                    ControlPanelTab::Clipboard => self.app.show_clipboard_tab(ui),
                    ControlPanelTab::TranslationService => self.app.show_translation_tab(ui),
                    ControlPanelTab::VoiceService => PanelApp::show_voice_tab(ui),
                }
            });
    }

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        tab.title().into()
    }

    fn is_closeable(&self, _tab: &Self::Tab) -> bool {
        false
    }
}

fn load_basic_config() -> (String, String) {
    let api_url = get_api();
    let theme = get_theme();
    let api_url = if api_url.trim().is_empty() {
        DEFAULT_API_URL.to_string()
    } else {
        api_url
    };
    (api_url, normalize_theme(&theme).to_string())
}

fn apply_theme(ctx: &Context, theme: &str) {
    ctx.set_theme(theme_preference(theme));
    install_fonts(ctx);
}

fn normalize_theme(theme: &str) -> &'static str {
    match theme {
        "system" => "system",
        "light" => "light",
        _ => "dark",
    }
}

fn theme_preference(theme: &str) -> egui::ThemePreference {
    match normalize_theme(theme) {
        "system" => egui::ThemePreference::System,
        "light" => egui::ThemePreference::Light,
        _ => egui::ThemePreference::Dark,
    }
}

fn theme_from_preference(preference: egui::ThemePreference) -> &'static str {
    match preference {
        egui::ThemePreference::System => "system",
        egui::ThemePreference::Light => "light",
        egui::ThemePreference::Dark => "dark",
    }
}

fn theme_preference_label(preference: egui::ThemePreference) -> &'static str {
    match preference {
        egui::ThemePreference::System => "跟随系统",
        egui::ThemePreference::Light => "浅色",
        egui::ThemePreference::Dark => "深色",
    }
}

fn section_title(ui: &mut egui::Ui, title: &str) {
    ui.label(RichText::new(title).strong());
}

fn hotkey_row(ui: &mut egui::Ui, label: &str, value: &mut String) {
    ui.horizontal(|ui| {
        ui.add_sized([120.0, 20.0], egui::Label::new(label));
        ui.add(egui::TextEdit::singleline(value).desired_width(260.0));
    });
}

fn readonly_row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.add_sized([120.0, 20.0], egui::Label::new(label));
        ui.label(RichText::new(value).color(ui.visuals().weak_text_color()));
    });
}

fn show_status(ui: &mut egui::Ui, status: &Option<Result<String, String>>) {
    let Some(status) = status else {
        return;
    };

    match status {
        Ok(message) => {
            ui.label(RichText::new(message).small().color(egui::Color32::GREEN));
        }
        Err(message) => {
            ui.label(RichText::new(message).small().color(egui::Color32::RED));
        }
    }
}

impl eframe::App for PanelApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.show_inside(ui);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_panel_tabs_have_expected_titles() {
        let titles = ControlPanelTab::ALL
            .into_iter()
            .map(ControlPanelTab::title)
            .collect::<Vec<_>>();

        assert_eq!(titles, ["通用", "快捷键", "剪切板", "翻译服务", "语音服务"]);
    }

    #[test]
    fn theme_strings_map_to_egui_preferences() {
        assert_eq!(theme_preference("system"), egui::ThemePreference::System);
        assert_eq!(theme_preference("light"), egui::ThemePreference::Light);
        assert_eq!(theme_preference("dark"), egui::ThemePreference::Dark);
        assert_eq!(theme_preference("unknown"), egui::ThemePreference::Dark);
    }

    #[test]
    fn egui_preferences_map_to_persisted_theme_strings() {
        assert_eq!(
            theme_from_preference(egui::ThemePreference::System),
            "system"
        );
        assert_eq!(theme_from_preference(egui::ThemePreference::Light), "light");
        assert_eq!(theme_from_preference(egui::ThemePreference::Dark), "dark");
    }
}
