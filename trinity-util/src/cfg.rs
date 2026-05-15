use std::{
    fs,
    path::{Path, PathBuf},
    sync::{LazyLock, Mutex},
};

use config::Config;
use log::warn;
use toml_edit::{DocumentMut, value};

use crate::hotkey::HotkeyConfig;

/// Global settings, lazily initialized
pub static SETTINGS: LazyLock<Mutex<Config>> = LazyLock::new(|| Mutex::new(Config::default()));

#[must_use]
pub fn settings_path() -> PathBuf {
    #[cfg(not(target_os = "windows"))]
    {
        PathBuf::from("/etc/translator/settings")
    }

    #[cfg(target_os = "windows")]
    {
        std::env::current_exe()
            .map(|path| match path.parent() {
                Some(parent) => parent.join("settings"),
                None => PathBuf::from("settings"),
            })
            .unwrap_or_else(|_| PathBuf::from("settings"))
    }
}

pub fn init_config() {
    let settings_path = settings_path();
    let settings_path = settings_path.to_string_lossy();

    let builder = Config::builder().add_source(config::File::with_name(&settings_path));
    match builder.build() {
        Ok(config) => *SETTINGS.lock().unwrap_or_else(|e| e.into_inner()) = config,
        Err(err) => warn!("settings merge failed, use default settings, err: {}", err),
    }
}

pub fn get_api() -> String {
    let settings = SETTINGS.lock().unwrap_or_else(|e| e.into_inner());
    settings
        .get_string("api")
        .unwrap_or("https://deepl.zu1k.com/translate".to_string())
}

pub fn get_window_size() -> (f32, f32) {
    let settings = SETTINGS.lock().unwrap_or_else(|e| e.into_inner());
    (
        settings.get_float("window.size.width").unwrap_or(500.0) as f32,
        settings.get_float("window.size.height").unwrap_or(200.0) as f32,
    )
}

pub fn get_theme() -> String {
    let settings = SETTINGS.lock().unwrap_or_else(|e| e.into_inner());
    settings
        .get_string("window.theme")
        .unwrap_or("dark".to_string())
}

#[must_use]
pub fn get_hotkey_config() -> HotkeyConfig {
    let settings = SETTINGS.lock().unwrap_or_else(|e| e.into_inner());
    HotkeyConfig {
        open_translator: settings
            .get_string("hotkey.open_translator")
            .or_else(|_| settings.get_string("hotkey.launch"))
            .unwrap_or_else(|_| HotkeyConfig::DEFAULT_OPEN_TRANSLATOR.to_string()),
        translate_selection: settings
            .get_string("hotkey.translate_selection")
            .unwrap_or_else(|_| HotkeyConfig::DEFAULT_TRANSLATE_SELECTION.to_string()),
        quit_app: settings
            .get_string("hotkey.quit_app")
            .or_else(|_| settings.get_string("hotkey.quit"))
            .unwrap_or_else(|_| HotkeyConfig::DEFAULT_QUIT_APP.to_string()),
    }
}

pub fn save_hotkey_config(config: &HotkeyConfig) -> std::io::Result<()> {
    save_hotkey_config_to_path(&settings_path(), config)?;
    init_config();
    Ok(())
}

fn save_hotkey_config_to_path(path: &Path, config: &HotkeyConfig) -> std::io::Result<()> {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(err),
    };
    let mut doc = content
        .parse::<DocumentMut>()
        .unwrap_or_else(|_| DocumentMut::new());

    doc["hotkey"]["open_translator"] = value(&config.open_translator);
    doc["hotkey"]["translate_selection"] = value(&config.translate_selection);
    doc["hotkey"]["quit_app"] = value(&config.quit_app);

    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, doc.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn save_hotkey_config_updates_only_hotkey_fields() {
        let path = temp_settings_path();
        fs::write(
            &path,
            r#"api = "https://example.test"

[hotkey]
launch = "Alt+Q"

[window]
theme = "light"
"#,
        )
        .unwrap_or_else(|err| panic!("failed to write test settings: {err}"));

        let config = HotkeyConfig {
            open_translator: "Alt+W".to_string(),
            translate_selection: "CmdOrCtrl+Shift+T".to_string(),
            quit_app: "CmdOrCtrl+Shift+D".to_string(),
        };
        save_hotkey_config_to_path(&path, &config)
            .unwrap_or_else(|err| panic!("failed to save hotkey config: {err}"));

        let saved = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("failed to read saved settings: {err}"));
        assert!(saved.contains(r#"api = "https://example.test""#));
        assert!(saved.contains(r#"theme = "light""#));
        assert!(saved.contains(r#"open_translator = "Alt+W""#));
        assert!(saved.contains(r#"translate_selection = "CmdOrCtrl+Shift+T""#));
        assert!(saved.contains(r#"quit_app = "CmdOrCtrl+Shift+D""#));

        let _ = fs::remove_file(path);
    }

    fn temp_settings_path() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        std::env::temp_dir().join(format!("trinity-settings-{nanos}.toml"))
    }
}
