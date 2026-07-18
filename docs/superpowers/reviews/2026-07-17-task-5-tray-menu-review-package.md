# Task 5 Tray Menu Review Package

**Date:** July 17, 2026  
**Scope:** Dynamic CodeSkin tray-theme menu only.

## Contract

- The `codeskin-tray` menu keeps **Show CodeSkin**, **Restore Codex original appearance**, and **Quit** actions, and adds a **主题** submenu.
- The submenu enumerates `ThemeLibrary.themes` in persisted library order. The item matching `selected_theme_id` is labelled `✓ {name}`.
- Each theme menu event uses the reversible `theme:{theme_id}` format. Empty IDs and IDs containing `:` are rejected before a menu item or application request is created.
- `sync_theme_menu(&AppHandle)` loads the library and constructs the complete replacement menu before finding the tray icon or calling `set_menu(Some(menu))`; a load/build failure therefore leaves the previous menu unchanged.
- A valid tray theme click calls shared Rust application logic rather than a Tauri command handler. That logic loads the library, resolves the theme or returns `theme_not_found`, applies it through the existing `AppState`, persists `selected_theme_id`, and starts the existing reconnection logic. The tray is synchronized only after this completes successfully.

## Changes

### `src-tauri/src/tray.rs`

- Added `build_menu(&App, &ThemeLibrary) -> tauri::Result<Menu<tauri::Wry>>` plus a private manager-generic constructor so both startup and `AppHandle` menu replacement use the same layout.
- Added `theme_menu_label` and `theme_event_id` pure helpers, with unit tests for selected labels, built-in/wallpaper-style IDs, and invalid empty/colon-containing IDs.
- Replaced the Task 4 synchronization placeholder with a fallible, single-argument `sync_theme_menu(&AppHandle)`.
- Added strict `theme:<id>` parsing in the tray event handler. Valid events use the shared helper in an async task and refresh the menu on success. Show, restore, and quit retain their existing Tauri APIs; quit restores first and then exits.
- Adds a disabled `没有已保存的主题` menu item if the library has no themes.

### `src-tauri/src/commands.rs`

- Extracted `pub(crate) async fn apply_saved_theme_by_id` from the `apply_theme` command. It has no Tauri UI-state dependency and is shared with the tray path.
- The command, wallpaper import, and rename paths now call the single-argument tray synchronizer so persisted selection/name changes are reflected in the next tray menu.

## Verification

1. `git rev-parse --is-inside-work-tree`
   - Exit code `128`: `fatal: not a git repository (or any of the parent directories): .git`.
   - No commit was created.
2. Test-first helper check before implementation:
   - `cargo test --manifest-path src-tauri/Cargo.toml tray::tests` initially failed with unresolved `theme_event_id` and `theme_menu_label` imports, as expected.
3. Focused helper tests after implementation:
   - `cargo test --manifest-path src-tauri/Cargo.toml tray::tests`
   - Result: `3 passed; 0 failed`.
4. Formatting and type check:
   - `cargo fmt --manifest-path src-tauri/Cargo.toml`
   - `cargo check --manifest-path src-tauri/Cargo.toml`
   - Result: success.
5. Full suite:
   - `cargo test --manifest-path src-tauri/Cargo.toml`
   - Result: `50 passed; 0 failed; 2 ignored` (the ignored tests require a running local Codex Desktop CDP instance).

The check/test commands retain three existing unrelated warnings in `storage` for an unused background import and unused legacy settings helpers.

## Limits and Constraint Review

- No frontend files, dependencies, `app_state.rs`, Codex files, `app.asar`, signatures, or listeners were changed.
- The implementation does not create network connections, change the existing CDP connection policy, connect to remote CDP endpoints, or terminate Codex. Theme application continues to use the existing `AppState` behavior and its existing local loopback/CDP safeguards.
- Unit tests cover the pure menu-label/event-ID contract. The native system-tray interaction itself was not run against a desktop shell during this task; full Rust tests include no live CDP execution because those two tests remain explicitly ignored.