# Task 6 — Theme Library GUI Review Package

**Review date:** July 17, 2026  
**Scope:** Frontend-only migration to the Task 6 theme-library contract. No Rust source, dependency manifest, or generated desktop artifact was changed.

## Contract coverage

- `ThemeSource` is `"builtin" | "wallpaper"`.
- `ThemeLayers` exposes camelCase numeric fields: `ambientOverlayOpacity`, `focusOverlayOpacity`, `sidebarOpacity`, and `cardOpacity`.
- `Theme` contains `source`, `layers`, and nullable `backgroundImage`; `ThemeLibrary` contains `version`, `selectedThemeId`, and `themes`.
- Rust `Option` response fields are modeled as required nullable properties (not optional): `CodexStatus.port`, `CodexStatus.executablePath`, and `VerifyResult.themeId` are `T | null`; `TargetVerification.mode` is `string | null` because Rust exposes `Option<String>`.
- The obsolete frontend `PersistedSettings` shape and old settings/background API wrappers were removed.
- The Tauri bindings use the new commands and camelCase parameters: `load_theme_library`, `import_wallpaper_theme({ bytes, displayName })`, and `rename_theme({ themeId, name })`. Apply, verify, restore, status inspection, and connection bindings remain available.

## Implementation review

- Application initialization calls `Promise.all([loadThemeLibrary(), inspectCodexStatus()])` and stores a single nullable `ThemeLibrary` state. The selected theme is derived from `selectedThemeId`; no independent legacy background-image state remains.
- Selecting a card updates `selectedThemeId` in the library state. Importing a file reads `File.arrayBuffer()`, converts it with `Array.from(new Uint8Array(bytes))`, imports with the original file name, and replaces the library with the returned selected library.
- Wallpaper themes render a real image element using `theme.backgroundImage`, `object-fit: cover`, a `WALLPAPER` source badge, and palette-derived color swatches. Built-in themes keep an intentionally abstract color preview. The UI does not recreate or show any simulated Codex workspace or full-window screenshot.
- The selected-theme panel displays ambient and focus overlay opacity as percentages. Wallpaper themes expose a labelled name input and a disabled-while-busy save action that calls `renameTheme` and retains the returned library.
- Verification target rows show target URL, detail, wallpaper/style layer state, and a mode chip. Connection, apply, verify, restore, busy/error feedback, and the non-official-tool notice remain in the interface.
- CSS adds responsive grid and mobile layouts, contrast-preserving controls, keyboard-visible focus styling, thumbnail cropping, badges, swatches, and verification chips.

## Validation evidence

1. Before frontend changes, ran `npm.cmd run build` from the workspace. It **passed** (`tsc && vite build`), so the requested record of an expected old-App failure is: no such failure was present; a prior build artifact/state did not mask a failure.
2. Confirmed `git rev-parse --is-inside-work-tree` exits with `128` and reports that this workspace is not a Git repository. No commit was attempted or created.
3. After the migration, ran `npm.cmd run build` successfully:
   - TypeScript compilation: passed.
   - Vite production build: passed.
4. Performed a source-level requirement review of the four permitted frontend files: no `PersistedSettings`, `loadSettings`, `getBuiltinThemes`, `importBackground`, or `clearBackground` wrapper/use remains; no Rust files or package manifests were modified.

## Limitations / follow-up manual validation

- No real desktop/Tauri runtime session was launched for this review, so actual CDP connection behavior, image decoding/persistence, the native file-picker path, and rendering against a live Codex window were not manually tested.
- The build validates TypeScript and production bundling but cannot independently exercise Tauri command serialization or inspect real wallpaper paths. Those should be checked in a desktop smoke test with one valid wallpaper import, rename, apply, verify, and restore cycle.

