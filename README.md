# Trinity

A lightweight desktop AI trifecta assistant built with Rust and egui, integrating word-selection translation, voice dictation, and clipboard management — targeting Windows, macOS, and Linux.

## Features

- **Word-Selection Translation** — select any text and get instant DeepL translation
- **Voice Dictation** — hold a global hotkey to record, transcribe with ElevenLabs, and type the result into the focused input
- **Clipboard Management** — persisted text history with a keyboard-driven picker
- **Control Panel** — configure API, hotkeys, theme, and service preferences
- Lightweight, minimal footprint, optimized binary size
- Automatic line-break cleanup for better PDF translation
- Cross-platform: Windows, macOS, Linux
- egui-based native UI, always-on-top floating window

## Architecture

```
trinity/
├── trinity/                # Binary entry point (main)
├── trinity-translator/     # Word-selection translation (lib)
├── trinity-clipboard/      # Clipboard history and picker (lib)
├── trinity-dictation/      # Voice dictation input (lib)
├── trinity-panel/          # Control panel (lib)
├── trinity-util/           # Shared utilities: config, fonts, icons
├── Cargo.toml              # Workspace configuration
└── LICENSE                 # MIT
```

| Crate | Role | Status |
|---|---|---|
| `trinity` | Binary entry, orchestrates all modules | Active |
| `trinity-translator` | DeepL translation, mouse/keyboard hooks, egui UI | Active |
| `trinity-clipboard` | Clipboard history & smart paste | Active |
| `trinity-dictation` | Voice recognition dictation | Active |
| `trinity-panel` | Control panel GUI | Active |
| `trinity-util` | Config, fonts, icons, theme helpers | Active |

## Usage

### Windows

Launch the binary — it stays in the background. Press `Alt+Q` to pop up the translator window. Press `Control+Shift+T` to translate the current selection. Press `Control+Shift+V` to open clipboard history. Hold `Control+Shift+Space` to dictate into the focused input. `Esc` closes floating windows. `Control+Shift+D` exits completely.

### macOS

Launch the binary. Press `Alt+Q` to pop up the translator window. Press `Command+Shift+T` to translate the current selection. Press `Command+Shift+V` to open clipboard history. Hold `Command+Shift+Space` to dictate into the focused input. `Esc` closes floating windows. `Command+Shift+D` exits completely.

### Linux

Launch the binary. Press `Alt+Q` to pop up the translator window. Press `Control+Shift+T` to translate the current selection. Press `Control+Shift+V` to open clipboard history. Hold `Control+Shift+Space` to dictate into the focused input. `Esc` closes floating windows. `Control+Shift+D` exits completely.

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
cargo run --release -p trinity

# Lint (strict)
cargo clippy --workspace -- -D warnings

# Format
cargo fmt --all

# Test
cargo test --workspace
```

Requires Rust nightly (edition 2024). The toolchain is pinned in `rust-toolchain.toml`.

## Configuration

Settings are loaded from `~/.trinity/config.json`. Trinity creates the file with defaults on first launch.

macOS example:

```json
{
  "api": "https://deepl.zu1k.com/translate",
  "window": {
    "size": {
      "width": 500.0,
      "height": 200.0
    },
    "font_size_plus": 0.0,
    "theme": "dark"
  },
  "hotkey": {
    "open_translator": "Alt+Q",
    "translate_selection": "Command+Shift+T",
    "open_clipboard": "Command+Shift+V",
    "record_dictation": "Command+Shift+Space",
    "quit_app": "Command+Shift+D"
  },
  "clipboard": {
    "capacity": 100,
    "panel_page_size": 10
  },
  "dictation": {
    "provider": "elevenlabs",
    "api_key": "YOUR_ELEVENLABS_API_KEY",
    "model_id": "scribe_v2",
    "language_code": null
  }
}
```

## License

This project is licensed under the [MIT License](./LICENSE).
