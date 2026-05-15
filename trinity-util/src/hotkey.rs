use std::collections::{HashMap, HashSet};

use global_hotkey::{
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
    hotkey::{HotKey, HotKeyParseError},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HotkeyAction {
    OpenTranslator,
    TranslateSelection,
    QuitApp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HotkeyConfig {
    #[serde(default = "default_open_translator")]
    pub open_translator: String,
    #[serde(default = "default_translate_selection")]
    pub translate_selection: String,
    #[serde(default = "default_quit_app")]
    pub quit_app: String,
}

impl HotkeyConfig {
    pub const DEFAULT_OPEN_TRANSLATOR: &'static str = "Alt+Q";
    pub const DEFAULT_TRANSLATE_SELECTION: &'static str = "CmdOrCtrl+Shift+T";
    pub const DEFAULT_QUIT_APP: &'static str = "CmdOrCtrl+Shift+D";

    #[must_use]
    pub fn entries(&self) -> [(HotkeyAction, &str); 3] {
        [
            (HotkeyAction::OpenTranslator, self.open_translator.as_str()),
            (
                HotkeyAction::TranslateSelection,
                self.translate_selection.as_str(),
            ),
            (HotkeyAction::QuitApp, self.quit_app.as_str()),
        ]
    }

    pub fn validate(&self) -> Result<(), HotkeyRegistrationError> {
        let parsed = parse_hotkeys(self)?;
        let mut ids = HashSet::new();
        for (action, hotkey) in parsed {
            if !ids.insert(hotkey.id()) {
                return Err(HotkeyRegistrationError::Duplicate {
                    action,
                    hotkey: hotkey.to_string(),
                });
            }
        }
        Ok(())
    }
}

fn default_open_translator() -> String {
    HotkeyConfig::DEFAULT_OPEN_TRANSLATOR.to_string()
}

fn default_translate_selection() -> String {
    HotkeyConfig::DEFAULT_TRANSLATE_SELECTION.to_string()
}

fn default_quit_app() -> String {
    HotkeyConfig::DEFAULT_QUIT_APP.to_string()
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            open_translator: Self::DEFAULT_OPEN_TRANSLATOR.to_string(),
            translate_selection: Self::DEFAULT_TRANSLATE_SELECTION.to_string(),
            quit_app: Self::DEFAULT_QUIT_APP.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HotkeyRegistrationError {
    Parse {
        action: HotkeyAction,
        hotkey: String,
        message: String,
    },
    Duplicate {
        action: HotkeyAction,
        hotkey: String,
    },
    Manager(String),
    Register {
        action: HotkeyAction,
        hotkey: String,
        message: String,
    },
    Unregister(String),
}

impl std::fmt::Display for HotkeyRegistrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse {
                action,
                hotkey,
                message,
            } => write!(f, "invalid hotkey for {action:?} ({hotkey}): {message}"),
            Self::Duplicate { action, hotkey } => {
                write!(f, "duplicate hotkey for {action:?}: {hotkey}")
            }
            Self::Manager(message) => write!(f, "hotkey manager unavailable: {message}"),
            Self::Register {
                action,
                hotkey,
                message,
            } => write!(f, "failed to register {action:?} ({hotkey}): {message}"),
            Self::Unregister(message) => write!(f, "failed to unregister hotkeys: {message}"),
        }
    }
}

impl std::error::Error for HotkeyRegistrationError {}

pub struct HotkeyService {
    manager: GlobalHotKeyManager,
    config: HotkeyConfig,
    registered: Vec<(HotkeyAction, HotKey)>,
    action_by_id: HashMap<u32, HotkeyAction>,
}

impl HotkeyService {
    pub fn new(config: &HotkeyConfig) -> Result<Self, HotkeyRegistrationError> {
        let manager = GlobalHotKeyManager::new()
            .map_err(|err| HotkeyRegistrationError::Manager(err.to_string()))?;
        let mut service = Self {
            manager,
            config: config.clone(),
            registered: Vec::new(),
            action_by_id: HashMap::new(),
        };
        service.register(config)?;
        Ok(service)
    }

    pub fn reload(&mut self, config: &HotkeyConfig) -> Result<(), HotkeyRegistrationError> {
        let old_config = self.config.clone();
        self.unregister_all()?;
        match self.register(config) {
            Ok(()) => {
                self.config = config.clone();
                Ok(())
            }
            Err(err) => {
                if let Err(restore_err) = self.register(&old_config) {
                    log::warn!(
                        "failed to restore previous hotkeys after reload error: {restore_err}"
                    );
                }
                Err(err)
            }
        }
    }

    #[must_use]
    pub fn poll_actions(&self) -> Vec<HotkeyAction> {
        let receiver = GlobalHotKeyEvent::receiver();
        let mut actions = Vec::new();
        while let Ok(event) = receiver.try_recv() {
            if event.state() == HotKeyState::Pressed
                && let Some(action) = self.action_by_id.get(&event.id())
            {
                actions.push(*action);
            }
        }
        actions
    }

    fn register(&mut self, config: &HotkeyConfig) -> Result<(), HotkeyRegistrationError> {
        let hotkeys = parse_hotkeys(config)?;
        let mut ids = HashSet::new();
        for (action, hotkey) in &hotkeys {
            if !ids.insert(hotkey.id()) {
                return Err(HotkeyRegistrationError::Duplicate {
                    action: *action,
                    hotkey: hotkey.to_string(),
                });
            }
        }

        for (action, hotkey) in hotkeys {
            if let Err(err) = self.manager.register(hotkey) {
                let _ = self.unregister_all();
                return Err(HotkeyRegistrationError::Register {
                    action,
                    hotkey: hotkey.to_string(),
                    message: err.to_string(),
                });
            }
            self.action_by_id.insert(hotkey.id(), action);
            self.registered.push((action, hotkey));
        }

        Ok(())
    }

    fn unregister_all(&mut self) -> Result<(), HotkeyRegistrationError> {
        let hotkeys = self
            .registered
            .iter()
            .map(|(_, hotkey)| *hotkey)
            .collect::<Vec<_>>();
        if !hotkeys.is_empty() {
            self.manager
                .unregister_all(&hotkeys)
                .map_err(|err| HotkeyRegistrationError::Unregister(err.to_string()))?;
        }
        self.registered.clear();
        self.action_by_id.clear();
        Ok(())
    }
}

impl Drop for HotkeyService {
    fn drop(&mut self) {
        let _ = self.unregister_all();
    }
}

fn parse_hotkeys(
    config: &HotkeyConfig,
) -> Result<Vec<(HotkeyAction, HotKey)>, HotkeyRegistrationError> {
    config
        .entries()
        .into_iter()
        .map(|(action, hotkey)| {
            hotkey
                .parse::<HotKey>()
                .map(|parsed| (action, parsed))
                .map_err(|err| parse_error(action, hotkey, err))
        })
        .collect()
}

fn parse_error(
    action: HotkeyAction,
    hotkey: &str,
    err: HotKeyParseError,
) -> HotkeyRegistrationError {
    HotkeyRegistrationError::Parse {
        action,
        hotkey: hotkey.to_string(),
        message: err.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_hotkeys_are_valid() {
        HotkeyConfig::default()
            .validate()
            .expect("default hotkeys should parse");
    }

    #[test]
    fn invalid_hotkey_is_rejected() {
        let config = HotkeyConfig {
            open_translator: "Shift+Ctrl".to_string(),
            ..Default::default()
        };

        assert!(matches!(
            config.validate(),
            Err(HotkeyRegistrationError::Parse {
                action: HotkeyAction::OpenTranslator,
                ..
            })
        ));
    }

    #[test]
    fn duplicate_hotkeys_are_rejected() {
        let config = HotkeyConfig {
            open_translator: "Alt+Q".to_string(),
            translate_selection: "Alt+Q".to_string(),
            quit_app: "CmdOrCtrl+Shift+D".to_string(),
        };

        assert!(matches!(
            config.validate(),
            Err(HotkeyRegistrationError::Duplicate { .. })
        ));
    }
}
