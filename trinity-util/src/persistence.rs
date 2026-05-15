use std::{
    fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::hotkey::HotkeyConfig;

const DEFAULT_API_URL: &str = "https://deepl.zu1k.com/translate";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_api")]
    pub api: String,
    #[serde(default)]
    pub window: WindowConfig,
    #[serde(default)]
    pub hotkey: HotkeyConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            api: default_api(),
            window: WindowConfig::default(),
            hotkey: HotkeyConfig::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindowConfig {
    #[serde(default)]
    pub size: WindowSize,
    #[serde(default = "default_font_size_plus")]
    pub font_size_plus: f32,
    #[serde(default = "default_theme")]
    pub theme: String,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            size: WindowSize::default(),
            font_size_plus: default_font_size_plus(),
            theme: default_theme(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WindowSize {
    #[serde(default = "default_window_width")]
    pub width: f32,
    #[serde(default = "default_window_height")]
    pub height: f32,
}

impl Default for WindowSize {
    fn default() -> Self {
        Self {
            width: default_window_width(),
            height: default_window_height(),
        }
    }
}

#[must_use]
pub fn config_path() -> PathBuf {
    home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".trinity")
        .join("config.json")
}

pub fn load_config() -> io::Result<AppConfig> {
    load_config_from_path(&config_path())
}

pub fn save_config(config: &AppConfig) -> io::Result<()> {
    save_config_to_path(&config_path(), config)
}

pub(crate) fn load_config_from_path(path: &Path) -> io::Result<AppConfig> {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            let config = AppConfig::default();
            save_config_to_path(path, &config)?;
            return Ok(config);
        }
        Err(err) => return Err(err),
    };

    let config = serde_json::from_str::<AppConfig>(&content)?;
    save_config_to_path(path, &config)?;
    Ok(config)
}

pub(crate) fn save_config_to_path(path: &Path, config: &AppConfig) -> io::Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }

    let content = serde_json::to_string_pretty(config)?;
    fs::write(path, format!("{content}\n"))
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

fn default_api() -> String {
    DEFAULT_API_URL.to_string()
}

fn default_theme() -> String {
    "dark".to_string()
}

fn default_font_size_plus() -> f32 {
    0.0
}

fn default_window_width() -> f32 {
    500.0
}

fn default_window_height() -> f32 {
    200.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn load_config_creates_default_file_when_missing() {
        let path = temp_config_path();

        let config = load_config_from_path(&path)
            .unwrap_or_else(|err| panic!("failed to load default config: {err}"));

        assert_eq!(config, AppConfig::default());
        assert!(path.exists());

        let _ = fs::remove_file(path);
    }

    #[test]
    fn partial_config_loads_with_defaults_and_rewrites_complete_json() {
        let path = temp_config_path();
        fs::create_dir_all(path.parent().unwrap_or_else(|| Path::new("")))
            .unwrap_or_else(|err| panic!("failed to create temp config dir: {err}"));
        fs::write(
            &path,
            r#"{"api":"https://example.test","window":{"theme":"light"}}"#,
        )
        .unwrap_or_else(|err| panic!("failed to write partial config: {err}"));

        let config = load_config_from_path(&path)
            .unwrap_or_else(|err| panic!("failed to load partial config: {err}"));

        assert_eq!(config.api, "https://example.test");
        assert_eq!(config.window.theme, "light");
        assert_eq!(config.window.size, WindowSize::default());
        assert_eq!(config.hotkey, HotkeyConfig::default());

        let saved = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("failed to read rewritten config: {err}"));
        assert!(saved.contains("open_translator"));
        assert!(saved.contains("font_size_plus"));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn invalid_json_returns_error() {
        let path = temp_config_path();
        fs::create_dir_all(path.parent().unwrap_or_else(|| Path::new("")))
            .unwrap_or_else(|err| panic!("failed to create temp config dir: {err}"));
        fs::write(&path, "{invalid json")
            .unwrap_or_else(|err| panic!("failed to write invalid config: {err}"));

        assert!(load_config_from_path(&path).is_err());

        let _ = fs::remove_file(path);
    }

    fn temp_config_path() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        std::env::temp_dir()
            .join(format!("trinity-config-{nanos}"))
            .join("config.json")
    }
}
