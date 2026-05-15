# Trinity

A lightweight desktop AI trifecta assistant built with Rust and egui, integrating word-selection translation, voice dictation, and clipboard management — targeting Windows, macOS, and Linux.

## Features

- **Word-Selection Translation** — select any text and get instant DeepL translation
- **Voice Dictation** — speech-to-text input method *(coming soon)*
- **Clipboard Management** — history, smart categorization *(coming soon)*
- **Settings Panel** — configure API, hotkeys, theme *(coming soon)*
- Lightweight, minimal footprint, optimized binary size
- Automatic line-break cleanup for better PDF translation
- Cross-platform: Windows, macOS, Linux
- egui-based native UI, always-on-top floating window

## Architecture

```
trinity/
├── trinity-gui/            # Binary entry point (main)
├── trinity-translator/     # Word-selection translation (lib)
├── trinity-clipboard/      # Clipboard management (lib, WIP)
├── trinity-dictation/      # Voice dictation input (lib, WIP)
├── trinity-panel/          # Settings and control panel (lib, WIP)
├── trinity-util/           # Shared utilities: config, fonts, icons
├── Cargo.toml              # Workspace configuration
└── LICENSE                 # MIT
```

| Crate | Role | Status |
|---|---|---|
| `trinity-gui` | Binary entry, orchestrates all modules | Active |
| `trinity-translator` | DeepL translation, mouse/keyboard hooks, egui UI | Active |
| `trinity-clipboard` | Clipboard history & smart paste | Stub |
| `trinity-dictation` | Voice recognition dictation | Stub |
| `trinity-panel` | Settings panel GUI | Stub |
| `trinity-util` | Config, fonts, icons, theme helpers | Active |

## Usage

### Windows

Launch the binary — it stays in the background. Press `Alt+Q` to pop up the translator window. `Esc` closes the window. `Ctrl+Shift+D` exits completely.

### macOS / Linux

Launch the binary — the translator window appears immediately. Select text anywhere to trigger translation.

### General

- Select text → automatic translation (word-selection mode)
- Language swap button (`⇌`) in the toolbar
- Manual translate button
- Window frame toggle (`□`) and drag (`○`)

## Build & Development

```bash
# Build all crates (release)
cargo build --workspace --release

# Run the application
cargo run --release -p trinity-gui

# Lint (strict)
cargo clippy --workspace -- -D warnings

# Format
cargo fmt --all

# Test
cargo test --workspace
```

Requires Rust nightly (edition 2024). The toolchain is pinned in `rust-toolchain.toml`.

## Configuration

Settings are loaded from a file:

- **Linux/macOS**: `/etc/translator/settings`
- **Windows**: `<exe_dir>/settings`

Example (`settings.toml`):

```toml
api = "https://deepl.zu1k.com/translate"

[window]
size.width = 500
size.height = 200
font_size_plus = 0
theme = "dark"    # "dark" or "light"

[hotkey]
launch = "ALT+Q"
quit = "CMDORCTRL+SHIFT+D"
```

## License

This project is licensed under the [MIT License](./LICENSE).