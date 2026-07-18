# Task 4 Theme Application Review Package

## Task contract

Implement the Rust/Tauri theme-library application flow without new dependencies or non-loopback CDP traffic. The implementation must:

- Treat `AppState.active_theme: Option<Theme>` as the only reconnect source and carry a complete `Theme` (including wallpaper and layers) through installation.
- Expose theme-library load, wallpaper import, apply, and wallpaper-only rename commands; retire the legacy settings/background commands from the Tauri command registration.
- Verify only CodeSkin-owned browser layers defensively, reporting missing `wallpaperLayer` and/or `styleLayer` markers as inactive.
- Keep restore behavior inactive, retain the ignored live-CDP verification test, and leave frontend files unchanged.

## Changed files

- `src-tauri/src/models.rs` — expanded `TargetVerification`; defensive browser-value decoding and unit coverage.
- `src-tauri/src/app_state.rs` — removed the separate background-image runtime state; added complete-theme apply/reconnect flow and enriched verification parsing; updated the ignored live CDP test.
- `src-tauri/src/commands.rs` — implemented the theme-library command surface, collision-safe wallpaper import, selected-theme persistence, wallpaper-only rename, and reconnect activation.
- `src-tauri/src/lib.rs` — registered the new commands and removed legacy UI command registrations.
- `src-tauri/src/tray.rs` — added the Task 4 `sync_theme_menu` compatibility shell for Task 5.

## Verification commands and output summary

- `cargo test --manifest-path src-tauri\Cargo.toml`
  - Passed: **47 passed, 0 failed, 2 ignored**; the ignored tests are the live Codex loopback-CDP checks.
  - The passing suite includes the exact browser verification JSON decode, missing-marker defense, wallpaper-ID collision handling, and all pre-existing unit tests.
- `cargo check --manifest-path src-tauri\Cargo.toml`
  - Passed with exit code 0.

Both commands emit three non-failing warnings from the intentionally retained legacy storage compatibility facade: one unused legacy background export plus unused `load_settings` and `save_settings` functions in storage files outside this task's permitted source-change scope.

## Known limitations

- `tray::sync_theme_menu` is intentionally a no-op compatibility shell; Task 5 owns the actual dynamic tray menu implementation.
- The existing storage compatibility facade (`load_settings`, `save_settings`, and the legacy background export) remains on disk for migration compatibility but is no longer exposed through Tauri's command handler.
- The live CDP test requires a user-run Codex Desktop instance with loopback CDP enabled and therefore remains `#[ignore]`.
