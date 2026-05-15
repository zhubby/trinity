//! Trinity Clipboard - text clipboard history and picker UI.

use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use cli_clipboard::{ClipboardContext, ClipboardProvider};
use egui::{Color32, RichText};
use log::warn;
use rdev::{EventType, Key, simulate};
use serde::{Deserialize, Serialize};
use trinity_util::{ClipboardConfig, cfg::clipboard_history_path};

/// Initialize the clipboard module.
pub fn init() {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardUiAction {
    None,
    Close,
    Paste(String),
}

#[derive(Clone)]
pub struct ClipboardManager {
    state: Arc<Mutex<ClipboardState>>,
    history_path: PathBuf,
    monitor_started: Arc<AtomicBool>,
}

impl ClipboardManager {
    #[must_use]
    pub fn new(config: ClipboardConfig) -> Self {
        Self::new_with_path(config, clipboard_history_path())
    }

    #[must_use]
    pub fn new_with_path(config: ClipboardConfig, history_path: PathBuf) -> Self {
        let config = config.normalized();
        let entries = load_history_from_path(&history_path)
            .map(|entries| trim_entries(entries, config.capacity))
            .unwrap_or_else(|err| {
                warn!("failed to load clipboard history: {err}");
                Vec::new()
            });

        Self {
            state: Arc::new(Mutex::new(ClipboardState::new(config, entries))),
            history_path,
            monitor_started: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn start_monitoring(&self) {
        if self.monitor_started.swap(true, Ordering::AcqRel) {
            return;
        }

        let state = self.state.clone();
        let history_path = self.history_path.clone();
        thread::spawn(move || monitor_clipboard(state, history_path));
    }

    pub fn reload_config(&self, config: ClipboardConfig) {
        let mut state = self.state.lock().unwrap_or_else(|err| err.into_inner());
        if state.reload_config(config)
            && let Err(err) = save_history_to_path(&self.history_path, state.entries())
        {
            warn!("failed to save clipboard history after config reload: {err}");
        }
    }

    #[must_use]
    pub fn show_inside(&self, ui: &mut egui::Ui) -> ClipboardUiAction {
        let mut state = self.state.lock().unwrap_or_else(|err| err.into_inner());
        state.handle_keyboard(ui);
        show_history(ui, &mut state)
    }

    #[must_use]
    pub fn entries(&self) -> Vec<String> {
        self.state
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .entries()
            .to_vec()
    }
}

#[derive(Debug, Clone)]
struct ClipboardState {
    config: ClipboardConfig,
    entries: Vec<String>,
    selected_index: usize,
    page: usize,
}

impl ClipboardState {
    fn new(config: ClipboardConfig, entries: Vec<String>) -> Self {
        let mut state = Self {
            config: config.normalized(),
            entries,
            selected_index: 0,
            page: 0,
        };
        state.normalize_selection();
        state
    }

    fn add_entry(&mut self, text: String) -> bool {
        let text = text.trim().to_string();
        if text.is_empty() {
            return false;
        }

        self.entries.retain(|entry| entry != &text);
        self.entries.insert(0, text);
        self.entries.truncate(self.config.capacity);
        self.selected_index = 0;
        self.page = 0;
        true
    }

    fn reload_config(&mut self, config: ClipboardConfig) -> bool {
        let config = config.normalized();
        let old_config = self.config;
        let old_len = self.entries.len();
        self.config = config;
        self.entries.truncate(self.config.capacity);
        self.normalize_selection();
        self.config != old_config || self.entries.len() != old_len
    }

    fn entries(&self) -> &[String] {
        &self.entries
    }

    fn selected_text(&self) -> Option<String> {
        self.entries.get(self.selected_index).cloned()
    }

    fn current_page_range(&self) -> std::ops::Range<usize> {
        page_range(self.entries.len(), self.config.panel_page_size, self.page)
    }

    fn page_count(&self) -> usize {
        page_count(self.entries.len(), self.config.panel_page_size)
    }

    fn handle_keyboard(&mut self, ui: &egui::Ui) {
        ui.input(|input| {
            if input.key_pressed(egui::Key::ArrowUp) {
                self.move_selection_up();
            }
            if input.key_pressed(egui::Key::ArrowDown) {
                self.move_selection_down();
            }
            if input.key_pressed(egui::Key::ArrowLeft) {
                self.previous_page();
            }
            if input.key_pressed(egui::Key::ArrowRight) {
                self.next_page();
            }
        });
    }

    fn move_selection_up(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        self.selected_index = self.selected_index.saturating_sub(1);
        self.page = self.selected_index / self.config.panel_page_size;
    }

    fn move_selection_down(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        self.selected_index = (self.selected_index + 1).min(self.entries.len() - 1);
        self.page = self.selected_index / self.config.panel_page_size;
    }

    fn previous_page(&mut self) {
        if self.page == 0 {
            return;
        }
        self.page -= 1;
        self.selected_index = self.page * self.config.panel_page_size;
        self.normalize_selection();
    }

    fn next_page(&mut self) {
        if self.page + 1 >= self.page_count() {
            return;
        }
        self.page += 1;
        self.selected_index = self.page * self.config.panel_page_size;
        self.normalize_selection();
    }

    fn normalize_selection(&mut self) {
        if self.entries.is_empty() {
            self.selected_index = 0;
            self.page = 0;
            return;
        }

        self.selected_index = self.selected_index.min(self.entries.len() - 1);
        self.page = self
            .page
            .min(self.page_count().saturating_sub(1))
            .min(self.selected_index / self.config.panel_page_size);
    }
}

fn show_history(ui: &mut egui::Ui, state: &mut ClipboardState) -> ClipboardUiAction {
    if ui.input(|input| input.key_pressed(egui::Key::Escape)) {
        return ClipboardUiAction::Close;
    }

    if ui.input(|input| input.key_pressed(egui::Key::Enter)) {
        return state
            .selected_text()
            .map(ClipboardUiAction::Paste)
            .unwrap_or(ClipboardUiAction::Close);
    }

    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            ui.heading("剪切板历史");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let page = if state.entries.is_empty() {
                    0
                } else {
                    state.page + 1
                };
                ui.label(
                    RichText::new(format!("{page}/{}", state.page_count()))
                        .small()
                        .color(ui.visuals().weak_text_color()),
                );
            });
        });
        ui.separator();

        if state.entries.is_empty() {
            ui.add_space(16.0);
            ui.centered_and_justified(|ui| {
                ui.label(RichText::new("暂无剪切板历史").color(ui.visuals().weak_text_color()));
            });
            return;
        }

        for index in state.current_page_range() {
            let selected = index == state.selected_index;
            let text = state.entries[index].clone();
            let preview = preview_text(&text);
            let fill = if selected {
                ui.visuals().selection.bg_fill
            } else {
                Color32::TRANSPARENT
            };
            let text_color = if selected {
                ui.visuals().selection.stroke.color
            } else {
                ui.visuals().text_color()
            };

            egui::Frame::NONE
                .fill(fill)
                .inner_margin(egui::Margin::same(8))
                .show(ui, |ui| {
                    let response = ui
                        .add_sized(
                            [ui.available_width(), 28.0],
                            egui::Label::new(RichText::new(preview).color(text_color)).truncate(),
                        )
                        .on_hover_text(text);
                    if response.clicked() {
                        state.selected_index = index;
                    }
                });
        }
    });

    ClipboardUiAction::None
}

pub fn paste_text(text: String) {
    if let Err(err) = set_clipboard_text(&text) {
        warn!("failed to set clipboard text before paste: {err}");
        return;
    }

    thread::sleep(Duration::from_millis(120));
    simulate_paste();
}

fn monitor_clipboard(state: Arc<Mutex<ClipboardState>>, history_path: PathBuf) {
    let mut last_seen = String::new();
    loop {
        match read_clipboard_text() {
            Ok(text) => {
                if text != last_seen {
                    last_seen = text.clone();
                    let mut state = state.lock().unwrap_or_else(|err| err.into_inner());
                    if state.add_entry(text)
                        && let Err(err) = save_history_to_path(&history_path, state.entries())
                    {
                        warn!("failed to save clipboard history: {err}");
                    }
                }
            }
            Err(err) => {
                warn!("failed to read clipboard: {err}");
            }
        }

        thread::sleep(Duration::from_millis(500));
    }
}

fn read_clipboard_text() -> io::Result<String> {
    let mut ctx: ClipboardContext =
        ClipboardProvider::new().map_err(|err| io::Error::other(err.to_string()))?;
    ctx.get_contents()
        .map_err(|err| io::Error::other(err.to_string()))
}

fn set_clipboard_text(text: &str) -> io::Result<()> {
    let mut ctx: ClipboardContext =
        ClipboardProvider::new().map_err(|err| io::Error::other(err.to_string()))?;
    ctx.set_contents(text.to_string())
        .map_err(|err| io::Error::other(err.to_string()))
}

fn simulate_paste() {
    let modifier = paste_modifier_key();
    _ = simulate(&EventType::KeyPress(modifier));
    _ = simulate(&EventType::KeyPress(Key::KeyV));
    _ = simulate(&EventType::KeyRelease(Key::KeyV));
    _ = simulate(&EventType::KeyRelease(modifier));
}

#[cfg(target_os = "macos")]
fn paste_modifier_key() -> Key {
    Key::MetaLeft
}

#[cfg(not(target_os = "macos"))]
fn paste_modifier_key() -> Key {
    Key::ControlLeft
}

#[derive(Debug, Serialize, Deserialize)]
struct HistoryFile {
    #[serde(default)]
    entries: Vec<String>,
}

fn load_history_from_path(path: &Path) -> io::Result<Vec<String>> {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err),
    };

    let history = serde_json::from_str::<HistoryFile>(&content)?;
    Ok(history
        .entries
        .into_iter()
        .filter(|entry| !entry.is_empty())
        .collect())
}

fn save_history_to_path(path: &Path, entries: &[String]) -> io::Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(&HistoryFile {
        entries: entries.to_vec(),
    })?;
    fs::write(path, format!("{content}\n"))
}

fn trim_entries(mut entries: Vec<String>, capacity: usize) -> Vec<String> {
    entries.truncate(capacity);
    entries
}

fn page_count(entry_count: usize, page_size: usize) -> usize {
    if entry_count == 0 {
        return 0;
    }
    entry_count.div_ceil(page_size.max(1))
}

fn page_range(entry_count: usize, page_size: usize, page: usize) -> std::ops::Range<usize> {
    if entry_count == 0 {
        return 0..0;
    }
    let page_size = page_size.max(1);
    let start = (page * page_size).min(entry_count);
    let end = (start + page_size).min(entry_count);
    start..end
}

fn preview_text(text: &str) -> String {
    let one_line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    const MAX_CHARS: usize = 120;
    if one_line.chars().count() <= MAX_CHARS {
        return one_line;
    }
    format!(
        "{}...",
        one_line.chars().take(MAX_CHARS).collect::<String>()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn add_entry_deduplicates_by_moving_existing_text_to_top() {
        let mut state = ClipboardState::new(
            ClipboardConfig {
                capacity: 10,
                panel_page_size: 3,
            },
            vec!["one".to_string(), "two".to_string(), "three".to_string()],
        );

        assert!(state.add_entry("two".to_string()));

        assert_eq!(state.entries(), ["two", "one", "three"]);
        assert_eq!(state.selected_index, 0);
        assert_eq!(state.page, 0);
    }

    #[test]
    fn add_entry_trims_to_capacity() {
        let mut state = ClipboardState::new(
            ClipboardConfig {
                capacity: 2,
                panel_page_size: 3,
            },
            Vec::new(),
        );

        state.add_entry("one".to_string());
        state.add_entry("two".to_string());
        state.add_entry("three".to_string());

        assert_eq!(state.entries(), ["three", "two"]);
    }

    #[test]
    fn pagination_handles_empty_and_last_page() {
        assert_eq!(page_count(0, 10), 0);
        assert_eq!(page_count(21, 10), 3);
        assert_eq!(page_range(0, 10, 0), 0..0);
        assert_eq!(page_range(21, 10, 0), 0..10);
        assert_eq!(page_range(21, 10, 2), 20..21);
    }

    #[test]
    fn history_json_round_trips_and_missing_file_starts_empty() {
        let path = temp_history_path();
        assert_eq!(
            load_history_from_path(&path).unwrap_or_default(),
            Vec::<String>::new()
        );

        let entries = vec!["alpha".to_string(), "beta".to_string()];
        save_history_to_path(&path, &entries)
            .unwrap_or_else(|err| panic!("failed to save history: {err}"));

        let loaded = load_history_from_path(&path)
            .unwrap_or_else(|err| panic!("failed to load history: {err}"));
        assert_eq!(loaded, entries);

        let _ = fs::remove_file(path);
    }

    fn temp_history_path() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir()
            .join(format!(
                "trinity-clipboard-{}-{nanos}-{counter}",
                std::process::id()
            ))
            .join("history.json")
    }
}
