use std::{
    path::{Path, PathBuf},
    sync::{LazyLock, Mutex},
};

use log::warn;

use crate::{
    hotkey::HotkeyConfig,
    persistence::{self, AppConfig, ClipboardConfig},
};

/// Global settings, lazily initialized from `~/.trinity/config.json`.
pub static SETTINGS: LazyLock<Mutex<AppConfig>> =
    LazyLock::new(|| Mutex::new(AppConfig::default()));

#[must_use]
pub fn settings_path() -> PathBuf {
    persistence::config_path()
}

#[must_use]
pub fn clipboard_history_path() -> PathBuf {
    persistence::clipboard_history_path()
}

pub fn init_config() {
    if let Err(err) = init_config_from_path(&settings_path()) {
        warn!("config load failed, using default settings, err: {}", err);
        *SETTINGS.lock().unwrap_or_else(|e| e.into_inner()) = AppConfig::default();
    }
}

pub fn get_api() -> String {
    SETTINGS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .api
        .clone()
}

pub fn get_window_size() -> (f32, f32) {
    let settings = SETTINGS.lock().unwrap_or_else(|e| e.into_inner());
    (settings.window.size.width, settings.window.size.height)
}

pub fn get_font_size_plus() -> f32 {
    SETTINGS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .window
        .font_size_plus
}

pub fn get_theme() -> String {
    SETTINGS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .window
        .theme
        .clone()
}

#[must_use]
pub fn get_clipboard_config() -> ClipboardConfig {
    SETTINGS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clipboard
        .normalized()
}

#[must_use]
pub fn get_hotkey_config() -> HotkeyConfig {
    SETTINGS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .hotkey
        .clone()
}

pub fn save_hotkey_config(config: &HotkeyConfig) -> std::io::Result<()> {
    save_hotkey_config_to_path(&settings_path(), config)?;
    init_config();
    Ok(())
}

pub fn save_basic_config(api_url: &str, theme: &str) -> std::io::Result<()> {
    save_basic_config_to_path(&settings_path(), api_url, theme)?;
    init_config();
    Ok(())
}

pub fn save_clipboard_config(config: ClipboardConfig) -> std::io::Result<()> {
    save_clipboard_config_to_path(&settings_path(), config.normalized())?;
    init_config();
    Ok(())
}

fn init_config_from_path(path: &Path) -> std::io::Result<()> {
    let config = persistence::load_config_from_path(path)?;
    *SETTINGS.lock().unwrap_or_else(|e| e.into_inner()) = config;
    Ok(())
}

fn save_basic_config_to_path(path: &Path, api_url: &str, theme: &str) -> std::io::Result<()> {
    let mut config = persistence::load_config_from_path(path)?;
    config.api = api_url.to_string();
    config.window.theme = theme.to_string();
    persistence::save_config_to_path(path, &config)
}

fn save_hotkey_config_to_path(path: &Path, hotkey: &HotkeyConfig) -> std::io::Result<()> {
    let mut config = persistence::load_config_from_path(path)?;
    config.hotkey = hotkey.clone();
    persistence::save_config_to_path(path, &config)
}

fn save_clipboard_config_to_path(path: &Path, clipboard: ClipboardConfig) -> std::io::Result<()> {
    let mut config = persistence::load_config_from_path(path)?;
    config.clipboard = clipboard.normalized();
    persistence::save_config_to_path(path, &config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::{ClipboardConfig, WindowConfig, WindowSize};
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn init_config_reads_json_values() {
        let path = temp_config_path();
        let config = AppConfig {
            api: "https://json.example.test".to_string(),
            window: WindowConfig {
                size: WindowSize {
                    width: 640.0,
                    height: 360.0,
                },
                font_size_plus: 2.0,
                theme: "light".to_string(),
            },
            hotkey: HotkeyConfig {
                open_translator: "Alt+W".to_string(),
                translate_selection: "Command+Shift+Y".to_string(),
                open_clipboard: "Command+Shift+V".to_string(),
                quit_app: "Command+Shift+U".to_string(),
            },
            clipboard: ClipboardConfig {
                capacity: 42,
                panel_page_size: 7,
            },
        };
        persistence::save_config_to_path(&path, &config)
            .unwrap_or_else(|err| panic!("failed to save test config: {err}"));

        init_config_from_path(&path).unwrap_or_else(|err| panic!("failed to init config: {err}"));

        assert_eq!(get_api(), "https://json.example.test");
        assert_eq!(get_theme(), "light");
        assert_eq!(get_window_size(), (640.0, 360.0));
        assert_eq!(get_font_size_plus(), 2.0);
        assert_eq!(get_hotkey_config(), config.hotkey);
        assert_eq!(get_clipboard_config(), config.clipboard);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn save_hotkey_config_updates_only_hotkey_fields() {
        let path = temp_config_path();
        let original = AppConfig {
            api: "https://example.test".to_string(),
            window: WindowConfig {
                theme: "light".to_string(),
                ..Default::default()
            },
            hotkey: HotkeyConfig {
                open_translator: "Alt+Q".to_string(),
                ..Default::default()
            },
            clipboard: ClipboardConfig {
                capacity: 25,
                panel_page_size: 4,
            },
        };
        persistence::save_config_to_path(&path, &original)
            .unwrap_or_else(|err| panic!("failed to save original config: {err}"));

        let hotkey = HotkeyConfig {
            open_translator: "Alt+W".to_string(),
            translate_selection: "Command+Shift+T".to_string(),
            open_clipboard: "Command+Shift+V".to_string(),
            quit_app: "Command+Shift+D".to_string(),
        };
        save_hotkey_config_to_path(&path, &hotkey)
            .unwrap_or_else(|err| panic!("failed to save hotkey config: {err}"));

        let saved = persistence::load_config_from_path(&path)
            .unwrap_or_else(|err| panic!("failed to read saved config: {err}"));
        assert_eq!(saved.api, original.api);
        assert_eq!(saved.window, original.window);
        assert_eq!(saved.hotkey, hotkey);
        assert_eq!(saved.clipboard, original.clipboard);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn save_basic_config_updates_only_api_and_theme() {
        let path = temp_config_path();
        let original = AppConfig {
            api: "https://old.example.test".to_string(),
            window: WindowConfig {
                size: WindowSize {
                    width: 700.0,
                    height: 300.0,
                },
                font_size_plus: 1.0,
                theme: "dark".to_string(),
            },
            hotkey: HotkeyConfig {
                open_translator: "Alt+Q".to_string(),
                translate_selection: "Command+Shift+T".to_string(),
                open_clipboard: "Command+Shift+V".to_string(),
                quit_app: "Command+Shift+D".to_string(),
            },
            clipboard: ClipboardConfig {
                capacity: 50,
                panel_page_size: 5,
            },
        };
        persistence::save_config_to_path(&path, &original)
            .unwrap_or_else(|err| panic!("failed to save original config: {err}"));

        save_basic_config_to_path(&path, "https://new.example.test", "light")
            .unwrap_or_else(|err| panic!("failed to save basic config: {err}"));

        let saved = persistence::load_config_from_path(&path)
            .unwrap_or_else(|err| panic!("failed to read saved config: {err}"));
        assert_eq!(saved.api, "https://new.example.test");
        assert_eq!(saved.window.theme, "light");
        assert_eq!(saved.window.size, original.window.size);
        assert_eq!(saved.window.font_size_plus, original.window.font_size_plus);
        assert_eq!(saved.hotkey, original.hotkey);
        assert_eq!(saved.clipboard, original.clipboard);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn save_clipboard_config_updates_only_clipboard_fields() {
        let path = temp_config_path();
        let original = AppConfig {
            api: "https://example.test".to_string(),
            window: WindowConfig {
                theme: "light".to_string(),
                ..Default::default()
            },
            hotkey: HotkeyConfig {
                open_translator: "Alt+Q".to_string(),
                ..Default::default()
            },
            clipboard: ClipboardConfig {
                capacity: 10,
                panel_page_size: 3,
            },
        };
        persistence::save_config_to_path(&path, &original)
            .unwrap_or_else(|err| panic!("failed to save original config: {err}"));

        let clipboard = ClipboardConfig {
            capacity: 0,
            panel_page_size: 1_000,
        };
        save_clipboard_config_to_path(&path, clipboard)
            .unwrap_or_else(|err| panic!("failed to save clipboard config: {err}"));

        let saved = persistence::load_config_from_path(&path)
            .unwrap_or_else(|err| panic!("failed to read saved config: {err}"));
        assert_eq!(saved.api, original.api);
        assert_eq!(saved.window, original.window);
        assert_eq!(saved.hotkey, original.hotkey);
        assert_eq!(
            saved.clipboard,
            ClipboardConfig {
                capacity: ClipboardConfig::MIN_CAPACITY,
                panel_page_size: ClipboardConfig::MAX_PANEL_PAGE_SIZE,
            }
        );

        let _ = fs::remove_file(path);
    }

    fn temp_config_path() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir()
            .join(format!(
                "trinity-cfg-{}-{nanos}-{counter}",
                std::process::id()
            ))
            .join("config.json")
    }
}
