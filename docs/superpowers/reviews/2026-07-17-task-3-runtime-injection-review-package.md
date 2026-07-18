# Task 3 Review Package — Runtime Wallpaper Layer, Glass Styling, and Mode Observer

Date: 2026-07-17

## Repository state

This workspace is not a Git repository. Inspect actual files directly; do not rely on a Git diff.

## In-scope files

- `src-tauri/src/injection/theme.rs`
- `src-tauri/src/injection/mod.rs`
- `src-tauri/src/injection-js/install.js`
- `src-tauri/src/injection-js/restore.js`
- `src-tauri/src/injection-js/verify.js`

## Security / product constraints

- Browser code must be original, must not modify Codex files, and must not use network APIs.
- The runtime layer must preserve native Codex controls: no `document.body.innerHTML`, no fake UI, no universal selector, and do not change control `display`, `position`, `width`, `height`, or `pointer-events`.
- Only CodeSkin-owned nodes/attributes/observer may be removed by restore.

## Required behavior

1. `install_expression(&Theme)` serializes camelCase layer properties, including `ambientOverlayOpacity` and `focusOverlayOpacity`.
2. `install.js` creates/reuses `#codeskin-wallpaper-layer`, sets `data-codeskin-owned=true` and `aria-hidden=true`, places it before the first body child, and creates a separate CodeSkin-owned style node rather than reusing arbitrary styles.
3. Wallpaper CSS must include fixed full-window, noninteractive values: `position: fixed`, `inset: 0`, `pointer-events: none`, `z-index: -2`, `background-size: cover`.
4. Compute mode conservatively using semantic main/composer/transcript/code/project selectors. Store `ambient` or `focus` in `data-codeskin-mode`; disconnect a prior `window.__codeskinModeObserver` before replacing it; defer to DOMContentLoaded if root/body missing.
5. Style only listed semantic element groups. No universal selector and none of the prohibited layout/input properties on Codex controls.
6. Restore disconnects/deletes only CodeSkin observer, removes two CodeSkin nodes and CodeSkin theme/mode attributes.
7. Verify returns `active`, `themeId`, `accent`, `wallpaperLayer`, `styleLayer`, `mode`; `active` requires both owned nodes, valid ID and `ambient|focus` mode.
8. Contract tests must cover wallpaper ownership/noninteraction/mode, payload layers, restore cleanup, verify fields, and no body HTML replacement.

## Claimed validation

- Red test before implementation: `cargo test --manifest-path src-tauri\\Cargo.toml injection::tests` (expected old-script contract failure)
- `cargo test --manifest-path src-tauri\\Cargo.toml injection`
- `cargo check --manifest-path src-tauri\\Cargo.toml`
- `node --check` on each injection JS file

## Reviewer deliverable

Review only; do not edit. Report separately: Spec compliance Approved/Rejected; Code quality Approved/Rejected; findings as Critical/Important/Minor with exact file and line numbers. Critical/Important block Task 4.
