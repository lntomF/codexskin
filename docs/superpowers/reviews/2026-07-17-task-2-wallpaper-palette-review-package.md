# Task 2 Review Package — Offline Wallpaper Palette and Theme Creation

Date: 2026-07-17

## Repository state

This workspace is not a Git repository. `git rev-parse --is-inside-work-tree` fails, so reviewers must inspect the named files directly rather than relying on a Git diff.

## Task contract

Implement offline wallpaper palette extraction and `Theme` generation without new dependencies, while keeping this task restricted to the storage layer.

## In-scope files

- `src-tauri/src/storage/palette.rs` (new)
- `src-tauri/src/storage/backgrounds.rs`
- `src-tauri/src/storage/mod.rs`
- `src-tauri/src/models.rs` only if strictly required (not expected)

## Required behavior

1. Export:
   - `generate_wallpaper_theme(wallpaper_path: String, display_name: String, image_bytes: &[u8]) -> Result<Theme, CommandError>`
   - `import_wallpaper_theme(bytes: &[u8], display_name: &str) -> Result<Theme, CommandError>`
2. Decode with the existing `image` crate; use a sample no larger than 64x64 and ignore pixels with alpha < 32.
3. Produce average RGB plus a saturated accent, using local relative brightness to select a safe palette.
4. Dark images must use foreground `#F4F7FF`, muted `#AAB4C7`, ambient opacity `.18`, focus opacity `.76`; light images must use `#172033`, `#536174`, `.28`, `.82`. Every CSS color is `#RRGGBB`; opacity stays in `[0,1]`.
5. Result is `ThemeSource::Wallpaper`, with `background_image: Some(path)` and a stable image-bytes-derived ID. No UUID dependency. Collision suffixing is intentionally deferred to Task 4.
6. Wallpaper import writes only under `%LOCALAPPDATA%\\CodeSkin\\wallpapers`, uses `image::guess_format` for extension, and never uses the user filename as a path. Keep size (12 MiB), dimension and PNG/JPEG/WebP validation.
7. Malformed bytes must return error code `background_decode_failed`.
8. Tests must cover dark, light, malformed data, and backgrounds import behavior.

## Out of scope

No changes to app state, Tauri commands, injection logic, tray, frontend, or dependency manifests.

## Claimed validation

- `cargo test --manifest-path src-tauri\\Cargo.toml storage::palette::tests`
- `cargo test --manifest-path src-tauri\\Cargo.toml storage::backgrounds::tests`
- `cargo check --manifest-path src-tauri\\Cargo.toml`

## Reviewer deliverable

Report separately:

- Spec compliance: Approved / Rejected
- Code quality: Approved / Rejected
- Findings classified Critical, Important, Minor, with exact file and line references.
- Critical or Important findings block Task 3. Review only; do not edit files.
