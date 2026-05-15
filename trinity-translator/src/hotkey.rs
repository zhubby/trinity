use cli_clipboard::{ClipboardContext, ClipboardProvider};
use rdev::{EventType, Key, simulate};
use std::{thread, time::Duration};

pub fn ctrl_c() -> Option<String> {
    _ = simulate(&EventType::KeyPress(Key::ControlLeft));
    _ = simulate(&EventType::KeyPress(Key::KeyC));
    _ = simulate(&EventType::KeyRelease(Key::KeyC));
    _ = simulate(&EventType::KeyRelease(Key::ControlLeft));

    thread::sleep(Duration::from_millis(200));

    let mut ctx: ClipboardContext = ClipboardProvider::new().ok()?;
    ctx.get_contents().ok()
}
