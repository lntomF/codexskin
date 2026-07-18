# Immersive CodeSkin Theme Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build offline-generated, persistent wallpaper themes that are injected as real visual layers into Codex while preserving every native Codex control and interaction.

**Architecture:** Extend the Rust theme domain from a selected built-in color to a persisted theme library with generated wallpaper themes. The existing loopback-only CDP client receives a richer theme payload; an original browser-side script adds only CodeSkin-owned wallpaper/style nodes and uses a DOM observer to choose ambient or focus mode. Tauri commands and the React UI manage import and selection; the tray reads the same library and applies a selected item immediately.

**Tech Stack:** Rust, Tauri 2, Tokio, tokio-tungstenite, serde/serde_json, image, React 19, TypeScript, Vite.

## Global Constraints

- CDP HTTP and WebSocket connections must only target `127.0.0.1`; never accept, forward, listen on, or connect to a non-loopback CDP address.
- Do not modify, repack, sign, or otherwise write to Codex official binaries, `app.asar`, or installation directories.
- Keep browser injection original; do not copy text from unlicensed third-party injection scripts.
- The injected wallpaper and CSS must leave Codex controls as genuine DOM controls. No screenshot overlay, click interceptor, synthetic sidebar, synthetic input, or `pointer-events` layer over user content.
- `restore` must delete only CodeSkin-owned DOM nodes, attributes, observer state, and CDP script registrations.
- If focus/ambient detection is uncertain, use focus styling and report the detected mode through `verify`.
- Use existing dependencies only: no new crate or JavaScript package is required for this scope.
- Windows is the validation platform. Do not claim macOS runtime validation without testing on macOS; keep the Tauri tray implementation portable.
- The workspace is not a Git repository. Run `git rev-parse --is-inside-work-tree` before any commit command; record that commits cannot be made if it returns nonzero.

---

## File Structure

| Path | Responsibility |
| --- | --- |
| `src-tauri/src/models.rs` | Serializable theme, source, layer, palette, and verification types shared by storage, CDP, commands, and UI. |
| `src-tauri/src/storage/themes.rs` | Versioned local theme-library read/write and migration from the old `settings.json` shape. |
| `src-tauri/src/storage/backgrounds.rs` | Local wallpaper file import, format validation, and image directory ownership. |
| `src-tauri/src/storage/palette.rs` | Offline pixel sampling and deterministic theme generation from a local wallpaper. |
| `src-tauri/src/storage/mod.rs` | Narrow public storage API. |
| `src-tauri/src/injection/theme.rs` | CDP payload serializer for the generated theme and layer parameters. |
| `src-tauri/src/injection-js/install.js` | Original DOM layer injection, DOM mode observer, and semantic CSS rules. |
| `src-tauri/src/injection-js/restore.js` | CodeSkin-owned DOM and observer teardown. |
| `src-tauri/src/injection-js/verify.js` | Browser-side inspection of nodes, attributes, theme ID, and mode. |
| `src-tauri/src/app_state.rs` | Resolves saved themes, installs them on targets, retains active theme for reconnect, and maps detailed verify results. |
| `src-tauri/src/commands.rs` | Tauri API for library load, wallpaper import/generation, applying a saved theme, rename/delete, verify, and restore. |
| `src-tauri/src/tray.rs` | Dynamic, persistent-theme submenu and immediate application callbacks. |
| `src-tauri/src/lib.rs` | Command registration and initial tray menu sync. |
| `src/types.ts` | TypeScript mirror of Rust serializable data. |
| `src/api.ts` | Typed invoke wrappers for new theme-library commands. |
| `src/App.tsx` | Theme-library state, import/create flow, selected-theme apply, rename, and detailed verify rendering. |
| `src/App.css` | Wallpaper thumbnail cards, source badges, mode/verification presentation, and responsive controls. |

---

### Task 1: Define the persisted theme-library domain and migrate existing settings

**Files:**
- Modify: `src-tauri/src/models.rs`
- Modify: `src-tauri/src/storage/themes.rs`
- Modify: `src-tauri/src/storage/mod.rs`
- Test: inline `#[cfg(test)]` modules in `src-tauri/src/models.rs` and `src-tauri/src/storage/themes.rs`

**Interfaces:**
- Consumes: existing `Theme`, `ThemeColors`, old `PersistedSettings { selected_theme_id, background_image }` JSON.
- Produces:
  ```rust
  pub enum ThemeSource { Builtin, Wallpaper }
  pub struct ThemeLayers {
      pub ambient_overlay_opacity: f32,
      pub focus_overlay_opacity: f32,
      pub sidebar_opacity: f32,
      pub card_opacity: f32,
  }
  pub struct Theme {
      pub id: String,
      pub name: String,
      pub description: String,
      pub colors: ThemeColors,
      pub background_image: Option<String>,
      pub source: ThemeSource,
      pub layers: ThemeLayers,
  }
  pub struct ThemeLibrary {
      pub version: u32,
      pub selected_theme_id: Option<String>,
      pub themes: Vec<Theme>,
  }
  pub fn load_theme_library() -> Result<ThemeLibrary, CommandError>;
  pub fn save_theme_library(library: &ThemeLibrary) -> Result<(), CommandError>;
  ```
- Produces: built-in themes carrying `ThemeSource::Builtin`, no wallpaper, and safe default layer opacities.

- [ ] **Step 1: Write migration and serialization tests**

  Add a test that deserializes the exact old settings JSON and asserts that migration preserves the selected ID and turns a saved background image into a valid wallpaper theme only when there is a selected built-in theme:

  ```rust
  #[test]
  fn migrates_legacy_selected_theme_and_background() {
      let legacy = br#"{\"selectedThemeId\":\"midnight-ink\",\"backgroundImage\":\"C:/CodeSkin/backgrounds/old.png\"}"#;
      let library = migrate_legacy_settings(legacy).expect("legacy settings migrate");
      let selected = library.selected_theme_id.as_deref().expect("selected migrated theme");
      let theme = library.themes.iter().find(|theme| theme.id == selected).unwrap();
      assert_eq!(library.version, THEME_LIBRARY_VERSION);
      assert_eq!(theme.source, ThemeSource::Wallpaper);
      assert_eq!(theme.background_image.as_deref(), Some("C:/CodeSkin/backgrounds/old.png"));
  }
  ```

  Add model tests that ensure all built-ins have a valid source, all layer alpha values are in `0.0..=1.0`, and wallpaper themes require a `Some(background_image)`.

- [ ] **Step 2: Run the new tests and verify they fail**

  Run:

  ```powershell
  cargo test --manifest-path src-tauri\Cargo.toml storage::themes::tests::migrates_legacy_selected_theme_and_background models::tests::builtin_theme_has_valid_layers
  ```

  Expected: compile failure because `ThemeLibrary`, `ThemeSource`, `ThemeLayers`, and migration helper do not yet exist.

- [ ] **Step 3: Implement the minimal domain model and versioned library**

  In `models.rs`, add serde camel-case types exactly as described above. Define defaults using:

  ```rust
  impl ThemeLayers {
      pub const fn builtin() -> Self {
          Self {
              ambient_overlay_opacity: 0.20,
              focus_overlay_opacity: 0.78,
              sidebar_opacity: 0.58,
              card_opacity: 0.46,
          }
      }
  }
  ```

  In `storage/themes.rs`, replace `PersistedSettings` file ownership with `ThemeLibrary`. Read `themes.json` first. If it is absent, read the old `settings.json`, migrate it in memory, write `themes.json`, and retain `settings.json` unchanged. A legacy selected built-in plus `backgroundImage` must become a new selected `ThemeSource::Wallpaper` theme that keeps the original built-in palette and layers; it must not select a no-wallpaper built-in and lose the image. If neither file exists, return a library containing `Theme::builtin()` and no selection. Reject a library whose `version` is greater than `THEME_LIBRARY_VERSION` with `theme_library_version_unsupported`.

  Keep `app_data_dir()` unchanged; create `theme_library_path()` as `app_data_dir()?.join("themes.json")`.

- [ ] **Step 4: Run storage and model tests**

  Run:

  ```powershell
  cargo test --manifest-path src-tauri\Cargo.toml storage::themes models::tests
  ```

  Expected: all targeted tests pass.

- [ ] **Step 5: Check repository state before commit**

  Run:

  ```powershell
  git rev-parse --is-inside-work-tree
  ```

  Expected in this workspace: nonzero with `fatal: not a git repository`; record that no commit is possible.

### Task 2: Generate a readable wallpaper theme entirely offline

**Files:**
- Create: `src-tauri/src/storage/palette.rs`
- Modify: `src-tauri/src/storage/backgrounds.rs`
- Modify: `src-tauri/src/storage/mod.rs`
- Modify: `src-tauri/src/models.rs`
- Test: inline `#[cfg(test)]` module in `src-tauri/src/storage/palette.rs`

**Interfaces:**
- Consumes: validated wallpaper bytes and a display name from the GUI.
- Produces:
  ```rust
  pub fn generate_wallpaper_theme(
      wallpaper_path: String,
      display_name: String,
      image_bytes: &[u8],
  ) -> Result<Theme, CommandError>;

  pub fn import_wallpaper_theme(
      bytes: &[u8],
      display_name: &str,
  ) -> Result<Theme, CommandError>;
  ```
- Produces: a deterministic base `ThemeSource::Wallpaper` ID from wallpaper bytes. The command layer resolves a collision against the loaded library with a numeric suffix; no UUID crate.

- [ ] **Step 1: Write palette tests with in-memory images**

  Build 8×8 images using the existing `image` crate and assert generated output has readable, bounded fields:

  ```rust
  #[test]
  fn dark_wallpaper_generates_light_foreground_and_focus_overlay() {
      let image = image::DynamicImage::new_rgb8(8, 8);
      let mut bytes = Vec::new();
      image.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png).unwrap();
      let theme = generate_wallpaper_theme("wallpapers/test.png".into(), "Test".into(), &bytes).unwrap();
      assert_eq!(theme.source, ThemeSource::Wallpaper);
      assert!(theme.background_image.is_some());
      assert!(theme.colors.foreground.starts_with('#'));
      assert!(theme.layers.focus_overlay_opacity >= theme.layers.ambient_overlay_opacity);
  }
  ```

  Add a bright-image counterpart that asserts a dark foreground and safe opacity range, plus a malformed-byte test expecting `background_decode_failed`.

- [ ] **Step 2: Run tests and verify they fail**

  Run:

  ```powershell
  cargo test --manifest-path src-tauri\Cargo.toml storage::palette::tests
  ```

  Expected: compile failure because `palette.rs` and `generate_wallpaper_theme` do not exist.

- [ ] **Step 3: Implement sampling and theme generation**

  In `palette.rs`:

  1. Decode with `image::load_from_memory`.
  2. Resize to at most `64 × 64` with `FilterType::Triangle`.
  3. Ignore pixels with alpha below `32`.
  4. Compute average RGB and a weighted accent candidate from pixels whose saturation is above `0.20`.
  5. Compute relative luminance using `0.2126*r + 0.7152*g + 0.0722*b` after normalizing RGB to `0.0..=1.0`.
  6. Use `#F4F7FF` / `#AAB4C7` foreground colors when luminance is below `0.45`; otherwise use `#172033` / `#536174`.
  7. Use `0.18` ambient and `0.76` focus overlays for dark imagery; `0.28` ambient and `0.82` focus overlays for light imagery.
  8. Clamp every opacity to `0.0..=1.0` and format CSS colors as uppercase `#RRGGBB`.

  In `backgrounds.rs`, retain size/format checks, write imported images beneath `CodeSkin/wallpapers`, and return both local path and original validated bytes to the generator. Use `image::guess_format` to choose an extension. Do not use the original filename for filesystem paths.

- [ ] **Step 4: Run tests and cargo check**

  Run:

  ```powershell
  cargo test --manifest-path src-tauri\Cargo.toml storage::palette::tests storage::backgrounds::tests
  cargo check --manifest-path src-tauri\Cargo.toml
  ```

  Expected: tests and check pass without adding dependencies.

- [ ] **Step 5: Check repository state before commit**

  Run the Task 1 git command. Expected: no repository; do not attempt a commit.

### Task 3: Inject wallpaper, semantic glass styling, and an automatic mode observer

**Files:**
- Modify: `src-tauri/src/injection/theme.rs`
- Modify: `src-tauri/src/injection/mod.rs`
- Modify: `src-tauri/src/injection-js/install.js`
- Modify: `src-tauri/src/injection-js/restore.js`
- Modify: `src-tauri/src/injection-js/verify.js`
- Test: inline Rust tests in `src-tauri/src/injection/mod.rs` and `src-tauri/src/injection/theme.rs`

**Interfaces:**
- Consumes: a complete `Theme` with `ThemeLayers`.
- Produces browser verification JSON:
  ```json
  {
    "active": true,
    "themeId": "wallpaper-…",
    "accent": "#AABBCC",
    "wallpaperLayer": true,
    "styleLayer": true,
    "mode": "ambient"
  }
  ```
- Produces: `install_expression(&Theme) -> Result<String, serde_json::Error>` with `layers` serialized as camel-case JSON.

- [ ] **Step 1: Write failing injection contract tests**

  Add tests that assert the install script includes all owned identifiers and no user-input interception:

  ```rust
  #[test]
  fn install_script_owns_a_noninteractive_wallpaper_layer() {
      assert!(INSTALL_SCRIPT.contains("codeskin-wallpaper-layer"));
      assert!(INSTALL_SCRIPT.contains("pointer-events: none"));
      assert!(INSTALL_SCRIPT.contains("data-codeskin-mode"));
      assert!(!INSTALL_SCRIPT.contains("document.body.innerHTML"));
  }

  #[test]
  fn payload_contains_generated_layer_parameters() {
      let mut themes = Theme::builtin();
      let theme = themes.remove(0);
      let expression = install_expression(&theme).unwrap();
      assert!(expression.contains("ambientOverlayOpacity"));
      assert!(expression.contains("focusOverlayOpacity"));
  }
  ```

  Add restore/verify contract tests for `codeskin-wallpaper-layer`, `data-codeskin-mode`, `wallpaperLayer`, and `styleLayer`.

- [ ] **Step 2: Run tests and verify they fail**

  Run:

  ```powershell
  cargo test --manifest-path src-tauri\Cargo.toml injection::tests
  ```

  Expected: the new assertions fail because the current script uses `body::before` and does not expose mode/layer state.

- [ ] **Step 3: Implement the original browser-side layer model**

  In `install.js`, create `ensureWallpaperLayer(root)` that creates only:

  ```js
  const wallpaper = document.getElementById("codeskin-wallpaper-layer") || document.createElement("div");
  wallpaper.id = "codeskin-wallpaper-layer";
  wallpaper.setAttribute("data-codeskin-owned", "true");
  wallpaper.setAttribute("aria-hidden", "true");
  ```

  Append this node before the first body child; CSS it with `position: fixed`, `inset: 0`, `pointer-events: none`, `z-index: -2`, `background-size: cover`, and a theme-specific background image. Add a separate owned style element, never reuse arbitrary existing style nodes.

  Implement `computeMode()` as a conservative score:

  ```js
  const hasMain = Boolean(document.querySelector("main, [role='main']"));
  const hasComposer = Boolean(document.querySelector("textarea, [contenteditable='true'], [role='textbox']"));
  const hasTranscript = Boolean(document.querySelector("[role='log'], [data-message-author-role]"));
  const hasCode = Boolean(document.querySelector("pre code, [data-language-for-alternating-lines]"));
  const hasProjectSurface = Boolean(document.querySelector("[role='tree'], [aria-label*='Project'], [aria-label*='项目']"));
  return hasMain && (hasTranscript || hasCode || (hasComposer && hasProjectSurface))
    ? "focus"
    : "ambient";
  ```

  Store the observer on `window.__codeskinModeObserver`; disconnect a prior CodeSkin observer before creating a new one. Its callback sets `data-codeskin-mode` after DOM mutations. If `document.body` or the root is absent, defer with `DOMContentLoaded` as the current script does.

  Style only semantic element groups behind the CodeSkin theme root: `main`, `nav`, `aside`, `[role='navigation']`, `[role='main']`, `[role='dialog']`, `[role='listbox']`, `[role='menu']`, `button`, inputs, textareas, `[contenteditable='true']`, `pre`, and `[role='button']`. Do not add a universal selector and do not change display, position, width, height, or pointer-event values of Codex controls.

  In `restore.js`, disconnect `window.__codeskinModeObserver`, delete that property, remove the two CodeSkin nodes, and remove both CodeSkin data attributes. In `verify.js`, report booleans for the two owned nodes and only report an active theme when both nodes, valid theme ID, and valid mode are present.

- [ ] **Step 4: Run injection tests**

  Run:

  ```powershell
  cargo test --manifest-path src-tauri\Cargo.toml injection::tests
  ```

  Expected: all injection contract tests pass.

- [ ] **Step 5: Check repository state before commit**

  Run the Task 1 git command. Expected: no repository; do not attempt a commit.

### Task 4: Thread saved themes and detailed verification through Rust state and Tauri commands

**Files:**
- Modify: `src-tauri/src/app_state.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/models.rs`
- Modify: `src-tauri/src/injection/registry.rs`
- Test: inline tests in `src-tauri/src/app_state.rs` and `src-tauri/src/commands.rs`

**Interfaces:**
- Consumes: `ThemeLibrary`, `Theme`, install-script verification JSON.
- Produces:
  ```rust
  pub struct TargetVerification {
      pub target_id: String,
      pub target_url: String,
      pub active: bool,
      pub detail: String,
      pub wallpaper_layer: bool,
      pub style_layer: bool,
      pub mode: Option<String>,
  }

  pub async fn apply_saved_theme(
      self: &Arc<Self>,
      theme: Theme,
  ) -> Result<VerifyResult, CommandError>;
  ```
- Produces Tauri commands:
  ```rust
  #[tauri::command]
  pub fn load_theme_library() -> Result<ThemeLibrary, CommandError>;

  #[tauri::command]
  pub async fn import_wallpaper_theme(
      bytes: Vec<u8>,
      display_name: String,
      state: State<'_, Arc<AppState>>,
      app: tauri::AppHandle,
  ) -> Result<ThemeLibrary, CommandError>;

  #[tauri::command]
  pub async fn apply_theme(theme_id: String, state: State<'_, Arc<AppState>>) -> Result<VerifyResult, CommandError>;

  #[tauri::command]
  pub fn rename_theme(
      theme_id: String,
      name: String,
      app: tauri::AppHandle,
  ) -> Result<ThemeLibrary, CommandError>;
  ```

- [ ] **Step 1: Write failing state and decoding tests**

  Add a unit test for `TargetVerification` decoding from the exact browser JSON and a live ignored test that asserts an applied generated theme returns `wallpaper_layer == true`, `style_layer == true`, and `mode` is `ambient` or `focus`:

  ```rust
  #[test]
  fn verification_decodes_wallpaper_and_mode() {
      let result = parse_verification_value(serde_json::json!({
          "active": true,
          "themeId": "wallpaper-test",
          "accent": "#112233",
          "wallpaperLayer": true,
          "styleLayer": true,
          "mode": "focus"
      }));
      assert!(result.active);
      assert_eq!(result.mode.as_deref(), Some("focus"));
      assert!(result.wallpaper_layer && result.style_layer);
  }
  ```

- [ ] **Step 2: Run tests and verify they fail**

  Run:

  ```powershell
  cargo test --manifest-path src-tauri\Cargo.toml app_state::tests::verification_decodes_wallpaper_and_mode
  ```

  Expected: compile failure because the new verification fields and decoder do not exist.

- [ ] **Step 3: Implement state ownership and command flow**

  Replace `RuntimeState.background_image` with `RuntimeState.active_theme: Option<Theme>` as the sole reconnect source. `apply_theme` must load `ThemeLibrary`, find `theme_id` across built-in and wallpaper entries, return `theme_not_found` if absent, call `apply_saved_theme`, update `selected_theme_id`, save the library, and start the existing reconnector.

  Keep the existing `install_on_all_targets` / `install_on_target` and page-load fallback. Pass a clone of the entire active `Theme` into those functions so reconnect always uses the selected wallpaper and layer settings.

  Extend the browser result parser defensively: absent `wallpaperLayer`, `styleLayer`, or `mode` means `false`, `false`, and `None`; no `unwrap` on remote CDP values. `detail` must say which owned marker is absent.

  Replace `load_settings`, `import_background`, and `clear_background` commands with the library commands defined above. `import_wallpaper_theme` imports bytes, generates a theme, resolves a unique ID against existing entries, adds it to the library, selects it, saves, sets it active in AppState, and invokes `tray::sync_theme_menu(&app)`. `rename_theme` trims the supplied name, rejects empty names with `theme_name_invalid`, updates only the matching saved wallpaper theme, persists the library, and synchronizes the tray menu.

- [ ] **Step 4: Register commands and run tests**

  Update `lib.rs` command registration only after implementing the new command functions. Run:

  ```powershell
  cargo test --manifest-path src-tauri\Cargo.toml
  cargo check --manifest-path src-tauri\Cargo.toml
  ```

  Expected: all non-ignored tests pass; live tests remain ignored by default.

- [ ] **Step 5: Check repository state before commit**

  Run the Task 1 git command. Expected: no repository; do not attempt a commit.

### Task 5: Build a dynamic tray submenu that immediately applies saved themes

**Files:**
- Modify: `src-tauri/src/tray.rs`
- Modify: `src-tauri/src/commands.rs`
- Test: inline tests in `src-tauri/src/tray.rs` for pure menu-label helpers

**Interfaces:**
- Consumes: `ThemeLibrary` and `Arc<AppState>`.
- Produces:
  ```rust
  pub fn build_menu(app: &App, library: &ThemeLibrary) -> tauri::Result<Menu<tauri::Wry>>;
  pub fn sync_theme_menu(app: &tauri::AppHandle) -> Result<(), CommandError>;
  ```
- Event IDs follow the exact reversible format `theme:<theme-id>`; `:` is rejected in generated theme IDs.

- [ ] **Step 1: Write menu helper tests**

  Extract a pure helper:

  ```rust
  fn theme_menu_label(theme: &Theme, selected_id: Option<&str>) -> String {
      if selected_id == Some(theme.id.as_str()) {
          format!("✓ {}", theme.name)
      } else {
          theme.name.clone()
      }
  }
  ```

  Test selected and unselected labels, and test that `theme_event_id("wallpaper-123") == "theme:wallpaper-123"`.

- [ ] **Step 2: Run tests and verify they fail**

  Run:

  ```powershell
  cargo test --manifest-path src-tauri\Cargo.toml tray::tests
  ```

  Expected: compile failure because the helper functions do not exist.

- [ ] **Step 3: Implement tray rebuild and immediate apply**

  Build the normal Show, Restore, Quit items plus a `主题` submenu containing every theme in library order. In `on_menu_event`, detect `theme:` IDs, resolve the exact ID, clone `Arc<AppState>`, and spawn an async call to `state.apply_theme(&theme_id)`. The state method persists the selection before returning.

  `sync_theme_menu` loads `ThemeLibrary`, calls `build_menu`, locates `codeskin-tray`, and calls `set_menu(Some(menu))`. Call it from setup after initial tray construction and from `import_wallpaper_theme`. On save failure, leave the previous menu intact and return the existing `CommandError` to the GUI command.

- [ ] **Step 4: Run tray tests and cargo check**

  Run:

  ```powershell
  cargo test --manifest-path src-tauri\Cargo.toml tray::tests
  cargo check --manifest-path src-tauri\Cargo.toml
  ```

  Expected: tests pass and the tray remains built with Tauri APIs only.

- [ ] **Step 5: Check repository state before commit**

  Run the Task 1 git command. Expected: no repository; do not attempt a commit.

### Task 6: Replace the single-background GUI with a theme-library workflow

**Files:**
- Modify: `src/types.ts`
- Modify: `src/api.ts`
- Modify: `src/App.tsx`
- Modify: `src/App.css`
- Test: `npm.cmd run build` as TypeScript and UI integration validation

**Interfaces:**
- Consumes:
  ```ts
  export type ThemeSource = "builtin" | "wallpaper";
  export type ThemeLayers = {
    ambientOverlayOpacity: number;
    focusOverlayOpacity: number;
    sidebarOpacity: number;
    cardOpacity: number;
  };
  export type Theme = {
    id: string;
    name: string;
    description: string;
    colors: ThemeColors;
    backgroundImage?: string | null;
    source: ThemeSource;
    layers: ThemeLayers;
  };
  export type ThemeLibrary = {
    version: number;
    selectedThemeId?: string | null;
    themes: Theme[];
  };
  export type TargetVerification = {
    targetId: string;
    targetUrl: string;
    active: boolean;
    detail: string;
    wallpaperLayer: boolean;
    styleLayer: boolean;
    mode?: "ambient" | "focus" | null;
  };
  ```
- Produces UI actions:
  ```ts
  loadThemeLibrary(): Promise<ThemeLibrary>
  importWallpaperTheme(bytes: number[], displayName: string): Promise<ThemeLibrary>
  renameTheme(themeId: string, name: string): Promise<ThemeLibrary>
  applyTheme(themeId: string): Promise<VerifyResult>
  ```

- [ ] **Step 1: Update TypeScript types and invoke wrappers first**

  Remove `PersistedSettings`, `importBackground`, and `clearBackground` wrappers. Add the exact theme-library types above and invoke wrappers:

  ```ts
  export const loadThemeLibrary = () => invoke<ThemeLibrary>("load_theme_library");
  export const importWallpaperTheme = (bytes: number[], displayName: string) =>
    invoke<ThemeLibrary>("import_wallpaper_theme", { bytes, displayName });
  export const renameTheme = (themeId: string, name: string) =>
    invoke<ThemeLibrary>("rename_theme", { themeId, name });
  ```

- [ ] **Step 2: Run TypeScript build and verify it fails**

  Run:

  ```powershell
  npm.cmd run build
  ```

  Expected: TypeScript errors in `App.tsx` because it still imports removed settings/background APIs.

- [ ] **Step 3: Implement the library-first screen**

  In `App.tsx`:

  1. Replace `themes`, `selectedId`, and `backgroundImage` initialization with one `ThemeLibrary` state and derived selected theme.
  2. On load, call `loadThemeLibrary()` and `inspectCodexStatus()` in parallel.
  3. Replace “导入背景图” with “导入壁纸并生成主题”; read `File.arrayBuffer()`, call `importWallpaperTheme(Array.from(new Uint8Array(bytes)), file.name)`, replace the library state with the response, and show the generated theme as selected.
  4. Render wallpaper-theme cards using a real `<img src={theme.backgroundImage}>` thumbnail inside the CodeSkin GUI, a `WALLPAPER` source badge, and derived color swatches. Built-ins continue using the existing abstract color preview.
  5. Show the selected theme’s `ambientOverlayOpacity` and `focusOverlayOpacity` as textual percentages, not an imitation Codex screenshot.
  6. For wallpaper themes, render an accessible name input initialized from `theme.name` and a “保存名称” action calling `renameTheme`; disable it while another command is busy and show the returned library state on success.
  7. Retain the existing Apply, Verify, Restore, connection, busy/error, and non-official-tool messaging.
  8. In Verify result rows, show wallpaper layer, style layer, and ambient/focus mode along with target URL and error detail.

  In `App.css`, add responsive card thumbnail styling with `object-fit: cover`, explicit card label contrast, a source badge, and a compact mode chip. Do not add any full-window screenshot or fake Codex workspace preview.

- [ ] **Step 4: Run frontend production build**

  Run:

  ```powershell
  npm.cmd run build
  ```

  Expected: `tsc && vite build` succeeds and emits `dist/index.html` plus bundled assets.

- [ ] **Step 5: Check repository state before commit**

  Run the Task 1 git command. Expected: no repository; do not attempt a commit.

### Task 7: Verify the complete desktop flow against real Codex

**Files:**
- Modify: `src-tauri/src/app_state.rs` ignored live test block
- Modify: `README.md` with Windows build/run and limitation notes
- Test: existing unit tests, ignored live CDP test, Tauri desktop build, manual tray and control interaction

**Interfaces:**
- Consumes: the completed library, injector, tray, and frontend.
- Produces: a release executable at `src-tauri/target/release/codeskin.exe` built by `npm.cmd run build:desktop`.

- [ ] **Step 1: Extend the ignored live CDP test**

  Update `applies_and_verifies_a_theme_on_live_codex` to create or select a local wallpaper theme, call `apply_theme`, and assert:

  ```rust
  assert!(result.active);
  assert!(result.targets.iter().all(|target| target.wallpaper_layer));
  assert!(result.targets.iter().all(|target| target.style_layer));
  assert!(result.targets.iter().all(|target| {
      matches!(target.mode.as_deref(), Some("ambient") | Some("focus"))
  }));
  ```

  Preserve its final `restore_theme()` call and assert `active == false` afterwards.

- [ ] **Step 2: Run all automated Rust validation**

  Run:

  ```powershell
  cargo test --manifest-path src-tauri\Cargo.toml
  cargo check --manifest-path src-tauri\Cargo.toml
  cargo test --manifest-path src-tauri\Cargo.toml live_cdp_tests::applies_and_verifies_a_theme_on_live_codex -- --ignored --nocapture
  ```

  Expected: normal tests pass; the ignored test connects only to the currently running local Codex instance and passes apply/verify/restore.

- [ ] **Step 3: Build the actual desktop package**

  Stop only `codeskin.exe` if it locks the release output; never stop `ChatGPT.exe` / Codex. Then run:

  ```powershell
  npm.cmd run build:desktop
  ```

  Expected: Tauri uses the production `frontendDist`, produces `src-tauri\target\release\codeskin.exe`, and does not depend on `http://localhost:1420`.

- [ ] **Step 4: Perform manual runtime validation**

  Launch the release executable. Validate each point against a real local Codex instance:

  1. Import a 16:9 PNG, JPEG, or WebP and confirm a new `WALLPAPER` theme card appears.
  2. Apply it and use Verify; every active target reports a wallpaper layer, style layer, theme ID, and one of `ambient` or `focus`.
  3. Click Codex sidebar entries, a suggestion card, project selection, and type in the input box. Confirm all remain native and interactive.
  4. Observe a welcome/home view, then enter a project or conversation. Confirm the mode changes to focus or verify reports focus after the work surface appears.
  5. Refresh or open another Codex page/window and wait for the existing load-event fallback. Verify the selected theme remains active.
  6. Use the Windows tray `主题` submenu to switch to another saved theme and confirm the change applies immediately.
  7. Select Restore and confirm the wallpaper node, style node, markers, and theme are absent.
  8. Restart Codex normally, wait for the reconnection loop, apply a saved theme again, and confirm the CodeSkin process remains responsive.

- [ ] **Step 5: Document outcomes and check repository state**

  Add the exact build command, loopback-only limitation, supported image types, restore behavior, and the fact that this is an unofficial visual-only tool to `README.md`. Run the Task 1 git command. Expected: no repository; do not attempt a commit.
