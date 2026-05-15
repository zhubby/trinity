//! Linux tray implementation using ksni (D-Bus StatusNotifierItem)
//!
//! Creates a system tray icon via the KDE/Freedesktop
//! StatusNotifierItem protocol. Runs in a background thread
//! communicating via D-Bus, and forwards tray events to
//! the main thread through `mpsc`.

use ksni::{Category, Status, ToolTip, menu::StandardItem};
use std::sync::{LazyLock, Mutex, mpsc};

/// Events emitted by the tray menu
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayEvent {
    ShowPanel,
    Exit,
}

/// Global sender — background thread writes tray events here.
static TRAY_TX: LazyLock<Mutex<Option<mpsc::Sender<TrayEvent>>>> =
    LazyLock::new(|| Mutex::new(None));

/// The ksni tray handler struct
struct TrinityTray;

impl ksni::Tray for TrinityTray {
    fn id(&self) -> String {
        "trinity".into()
    }

    fn title(&self) -> String {
        "Trinity".into()
    }

    fn status(&self) -> Status {
        Status::Active
    }

    fn category(&self) -> Category {
        Category::ApplicationStatus
    }

    fn icon_name(&self) -> String {
        // Use a generic icon name; we'll try to set icon pixmap from PNG
        "accessories-dictionary".into()
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            title: "Trinity".into(),
            description: "Desktop AI trifecta assistant".into(),
            icon_name: "accessories-dictionary".into(),
            icon_pixmap: vec![], // could embed PNG data here
        }
    }

    fn menu(&self) -> ksni::menu::Menu<TrinityTray> {
        ksni::menu::Menu {
            label: "Trinity".into(),
            submenu: vec![
                ksni::menu::MenuItem::StandardItem(StandardItem {
                    label: "Show Settings Panel".into(),
                    icon_name: "preferences-system".into(),
                    enabled: true,
                    clicked: Box::new(|_this| {
                        let guard = TRAY_TX.lock().unwrap_or_else(|e| e.into_inner());
                        if let Some(sender) = guard.as_ref() {
                            sender.send(TrayEvent::ShowPanel).ok();
                        }
                    }),
                    ..Default::default()
                }),
                ksni::menu::MenuItem::Separator,
                ksni::menu::MenuItem::StandardItem(StandardItem {
                    label: "Exit".into(),
                    icon_name: "application-exit".into(),
                    enabled: true,
                    clicked: Box::new(|_this| {
                        let guard = TRAY_TX.lock().unwrap_or_else(|e| e.into_inner());
                        if let Some(sender) = guard.as_ref() {
                            sender.send(TrayEvent::Exit).ok();
                        }
                    }),
                    ..Default::default()
                }),
            ],
            ..Default::default()
        }
    }
}

// ── Public API ──────────────────────────────────────────────────────

/// Create the Linux system tray icon and menu.
///
/// Spawns a background thread that runs the D-Bus StatusNotifierItem
/// service. Tray events are forwarded via the returned `Receiver`.
pub fn create_tray(_ctx: egui::Context, _tx: mpsc::Sender<TrayEvent>) -> mpsc::Receiver<TrayEvent> {
    let (tx, rx) = mpsc::channel();
    *TRAY_TX.lock().unwrap_or_else(|e| e.into_inner()) = Some(tx);

    std::thread::spawn(|| {
        let tray = TrinityTray;
        let service = ksni::TrayService::new(tray);
        service.run();
    });

    rx
}
