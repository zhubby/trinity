# Repository Guidelines

## Project Structure & Module Organization

This repository is a Rust workspace. Crates are split by responsibility:

- `trinity` (directory `trinity/`): binary entry point, orchestrates all modules at startup.
- `trinity-translator`: word-selection translation engine — DeepL API, mouse/keyboard hooks, egui UI, platform-specific run loops.
- `trinity-clipboard`: clipboard management GUI (history, smart paste) — currently a stub.
- `trinity-dictation`: voice dictation GUI / speech-to-text input — currently a stub.
- `trinity-panel`: settings and control panel GUI — currently a stub.
- `trinity-util`: shared utilities — config loading, font installation, icon loading, theme helpers.

Each GUI module (`translator`, `clipboard`, `dictation`, `panel`) is an independent egui-based panel that can be launched by `trinity` (the binary crate in `trinity/`). Shared infrastructure (fonts, icons, config, theme) lives in `trinity-util` so no GUI crate duplicates resource-loading logic.

Keep new code in the crate that owns the domain concern. Avoid cross-crate leakage of UI-specific logic into utility crates, and avoid embedding platform-specific lifecycle code outside the `cfg(target_os)` boundaries in `trinity-translator`.

Each lib crate must expose a clean public API through `lib.rs` with `pub use` re-exports. The binary crate (`trinity`, in directory `trinity/`) is the sole entry point and must not contain business logic — it delegates to the lib crates.

## Build, Test, and Development Commands

Use workspace-level Cargo commands from repo root:

- `cargo check --workspace`: fast compile verification.
- `cargo build --workspace --release`: build all crates (release profile with LTO, strip, `opt-level = "s"`).
- `cargo test --workspace`: run unit and integration tests.
- `cargo fmt --all`: apply Rust formatting (style edition 2024).
- `cargo clippy --workspace -- -D warnings`: lint strictly, zero warnings required.

Requires Rust **nightly** channel (edition 2024). The toolchain is pinned in `rust-toolchain.toml` with `rustfmt` and `clippy` components.

## Rust Style and Idioms

- Target Rust **2024 edition**. Use edition-aware idioms (`let`-chains in `if`, `LazyLock` instead of `lazy_static!`, destructuring assignment, etc.).
- Use `cfg_if::cfg_if!` for platform branching — keep `#[cfg(target_os = "...")]` annotations minimal and grouped.
- Derive `Default` when all fields have sensible defaults.
- Use `std::sync::LazyLock` for global statics — no `lazy_static!` or `once_cell` in new code.
- Prefer guard clauses (early returns) and `let`-chains over nested `if` blocks.
- Prefer `let-else` when destructuring must succeed and the failure path should return/continue/break.
- Prefer `Option`/`Result` combinators (`is_some_and`, `then_some`, `transpose`, `inspect`) when they keep ownership obvious; switch to `match` once closure logic becomes non-trivial.
- Prefer iterators/combinators over manual loops.
- Keep public API surfaces small. Use `#[must_use]` where return values matter.
- **Never `.unwrap()` / `.expect()` in production paths.** Use `?`, `ok_or_else`, `unwrap_or_default`, `unwrap_or_else`. Lock poison recovery: `unwrap_or_else(|e| e.into_inner())`.
- Use concrete types (`struct`/`enum`) over stringly-typed values wherever shape is known. Only convert to strings at display/serialization boundaries.
- Prefer `From`/`Into`/`TryFrom`/`TryInto` over manual conversions.
- Forbidden: `Mutex<()>` / `Arc<Mutex<()>>` — mutex must guard actual state.

## Workspace Dependency Management

All crates share a single source of truth for dependencies in the root `Cargo.toml`:

- **All dependencies must be declared in `[workspace.dependencies]`** at the repository root.
- Individual crates reference workspace dependencies using `{ workspace = true }` syntax.
- Path-based internal crates must also use `{ workspace = true }`.
- Optional/feature-gated dependencies use `{ workspace = true, optional = true }`.
- When adding features to a workspace dependency in a crate, use `{ workspace = true, features = [...] }`.

Example:

```toml
# Root Cargo.toml
[workspace.dependencies]
eframe = "0.34"
egui = "0.34"
config = "0.15"

# Sub-crate Cargo.toml
[dependencies]
eframe = { workspace = true }
config = { workspace = true }
```

## Shared Utilities (trinity-util)

`trinity-util` provides infrastructure shared across all GUI crates:

- **`cfg`**: config loading (`init_config`), global settings (`SETTINGS`), config key accessors (`get_api`, `get_window_size`, `get_theme`).
- **`font`**: font installation (`install_fonts`) and style customization with configurable font size offset.
- **`icon`**: window icon loading from embedded PNG (`get_icon_data`).

Rules for `trinity-util`:

- Only extract code into `trinity-util` when it is genuinely shared (used by ≥2 crates) or is a foundational piece (config, fonts) that every module needs.
- Do not move module-specific business logic (translation, clipboard operations, hotkey registration) into `trinity-util`.
- `trinity-util` must remain platform-agnostic: no `cfg(target_os)` platform branching inside it (that belongs in the consuming crate).
- Resource files (`assets/` directory) live inside `trinity-util` so all crates can `include_bytes!` from a single location via path `../assets/...`.

## egui / eframe Conventions (0.34)

All GUI modules use `eframe` 0.34 / `egui` 0.34. Follow these rules when modifying UI code:

- Use `eframe::App::ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame)` — the `update` method is gone.
- Access input through closure pattern: `ctx.input(|i| i.key_pressed(...))`.
- Window operations via `ctx.send_viewport_cmd(ViewportCommand::...)` — never use the old `frame.close()`, `frame.set_decorations()`, or `frame.drag_window()`.
- Use `ViewportBuilder` for `NativeOptions` configuration — old flat fields (`always_on_top`, `decorated`, `initial_window_size`) are gone.
- Use `CentralPanel::show_inside(ui, ...)` inside `App::ui` — the `show(ctx, ...)` variant is deprecated.
- Use `ComboBox::from_id_salt(...)` — `from_id_source` is renamed.
- Use `egui::Frame::NONE` instead of `Frame::none()` — the latter is deprecated.
- Use `ctx.global_style()` / `ctx.set_global_style(...)` — `style()` / `set_style()` are deprecated.
- Wrap `FontData` in `Arc` when inserting into `FontDefinitions`.
- The `eframe::run_native` creator closure returns `Result<Box<dyn App>, ...>` — always `Ok(...)` unless init genuinely fails.
- Icons are set via `ViewportBuilder::with_icon(...)` with `egui::IconData`, using `trinity_util::icon::get_icon_data()`.
- Fonts and theme are applied via `trinity_util::font::install_fonts()` and `trinity_util::cfg::get_theme()`.
- Do not block the egui render/update path on IO work. All translation and clipboard operations run on background threads with `Arc<Mutex<State>>` shared state and `request_repaint()` polling.

## Platform-Specific Code

Platform differences are confined to `trinity-translator/src/unix.rs` and `trinity-translator/src/windows.rs`:

- **Unix/macOS**: `run()` calls `init_config()` then `eframe::run_native` directly.
- **Windows**: standalone translator hotkeys use the shared `trinity_util::hotkey` service; the main `trinity` daemon (in `trinity/`) owns application-wide hotkey registration and panel-triggered reloads.

Do not spread platform `cfg` annotations across multiple files. When adding new platform-specific behavior, extend the existing `unix.rs` / `windows.rs` split or use `cfg_if::cfg_if!` in `lib.rs`.

## Testing Guidelines

Place unit tests next to implementation (`mod tests`). Integration tests under `*/tests/` when they arise.

Name tests by behavior: `translate_returns_error_on_invalid_api`, `mouse_state_detects_selection_sequence`.

Every modification should keep the workspace test suite passing. Add regression tests for bug fixes.

## Configuration Persistence

Settings are loaded from a platform-specific path (`/etc/translator/settings` on Unix, `<exe_dir>/settings` on Windows) via the `config` crate into a `LazyLock<Mutex<Config>>` global in `trinity_util::cfg`.

- Read config through `SETTINGS.lock().unwrap()` — lock is unpoisonable in practice (only panics on init failure).
- Do not persist config changes in the current architecture. If future modules need mutable config, prefer a shared `ConfigStore` with targeted mutation + validate + write, never blanket overwrite from stale snapshots.

## Commit & Pull Request Guidelines

Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <subject>

<body>

<footer>
```

### Commit Types

| Type       | Description                                 |
| ---------- | ------------------------------------------- |
| `feat`     | New feature                                 |
| `fix`      | Bug fix                                     |
| `docs`     | Documentation changes                       |
| `style`    | Code style (formatting, semicolons, etc.)   |
| `refactor` | Code refactoring without behavior change    |
| `perf`     | Performance improvements                    |
| `test`     | Test additions or corrections               |
| `chore`    | Maintenance tasks, dependencies, tooling    |
| `ci`       | CI/CD configuration changes                 |
| `build`    | Build system or external dependency changes |
| `revert`   | Reverting a previous commit                 |

Subject line: imperative mood, lowercase, no trailing period, max 72 chars.

PRs should include:

- Purpose and impacted crates.
- Test evidence (commands run + results).
- Config/doc updates when behavior changes.

### Examples

```
feat(translator): add language auto-detect fallback

fix(gui): resolve window not appearing on macOS launch

refactor: extract shared utilities into trinity-util

chore(deps): upgrade egui to 0.34 and migrate API
```

## Module Documentation & Changelog

Each workspace crate should maintain its own documentation:

- **`README.md`** (at crate root): describe module capabilities, implementation notes, and architecture.
- **`CHANGELOG.md`** (at crate root): record changes on every modification. Format with date and type (`Added` / `Changed` / `Fixed` / `Removed`).

Keep documentation in sync with code — update when descriptions become inaccurate.

## Security

Never commit API keys or credentials. Translation API endpoints are configured via the settings file. If sharing configs, redact sensitive values.
