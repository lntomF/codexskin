# CodeSkin Runtime Theme Tool Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Windows Tauri desktop application that starts or discovers a local-CDP Codex instance and applies, verifies, restores, and persists non-official visual themes without modifying Codex files.

**Architecture:** Rust owns process discovery, strict-loopback CDP discovery, a compact JSON-RPC WebSocket client, injection lifecycle, persistence, and tray behavior. The existing React frontend is replaced with a stateful theme-grid interface that invokes a small typed command surface. The injection JavaScript is independently authored and only adds/removes CodeSkin-owned DOM nodes and attributes.

**Tech Stack:** Tauri 2, React 19, TypeScript, Tokio, tokio-tungstenite, serde/serde_json, sysinfo, image, futures-util.

## Global Constraints

- CDP discovery and WebSocket connections must only use `127.0.0.1`; reject all other hosts and never proxy a debugging port.
- Do not modify Codex installation files, `app.asar`, signatures, or package contents.
- Do not reuse `injector.mjs`: its source and licence are absent from this workspace; all injection JavaScript must be newly written.
- Provide `verify` and `restore`, not only injection.
- Never terminate a running `Codex.exe`; an already-running instance with no debug port must be reported to the user.
- Application UI must state that this is a non-official visual customization tool.
- No Node.js runtime is required by the packaged executable. Vite/Node may be used only at build time.
- New direct dependencies are limited to `tokio-tungstenite`, `image`, and `futures-util`; retain direct `serde`, `serde_json`, and `sysinfo` dependencies.

---

### Task 1: Establish crates, models, commands, and a buildable UI shell

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/tauri.conf.json`
- Create: `src-tauri/src/error.rs`
- Create: `src-tauri/src/models.rs`
- Create: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/App.tsx`
- Modify: `src/App.css`
- Create: `src/api.ts`
- Create: `src/types.ts`

**Interfaces:**
- Produces `Theme`, `CodexStatus`, `VerifyResult`, `CommandError`, and command names consumed by the React frontend.
- Produces a `tauri::State<AppState>` placeholder that later tasks populate.

- [ ] **Step 1: Write failing model unit tests**

```rust
#[test]
fn builtin_theme_has_nonempty_css_values() {
    let theme = Theme::builtin()[0].clone();
    assert!(theme.colors.accent.starts_with('#'));
    assert!(!theme.id.is_empty());
}
```

- [ ] **Step 2: Run the focused test to confirm the type is absent**

Run: `cargo test models::tests::builtin_theme_has_nonempty_css_values`
Expected: compilation failure because `models` and `Theme` do not exist.

- [ ] **Step 3: Add exact dependencies and minimal typed models**

```toml
[dependencies]
tauri = { version = "2.0", features = ["tray-icon"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokio = { version = "1.0", features = ["full"] }
tokio-tungstenite = "0.24"
futures-util = "0.3"
sysinfo = "0.30"
image = "0.25"
```

Implement serializable DTOs and `Theme::builtin()` with three fixed themes. Register a minimal `get_builtin_themes` command, keep state construction compilable, and replace the Vite greeting screen with a title, connection badge, static theme cards, and non-official notice.

- [ ] **Step 4: Run models test and frontend production build**

Run: `cargo test models::tests::builtin_theme_has_nonempty_css_values && npm run build`
Expected: both commands exit with status 0.

- [ ] **Step 5: Record verification result in this plan**

Mark this task only after the two commands in Step 4 have fresh successful output.

### Task 2: Implement safe Codex process discovery and launch preparation

**Files:**
- Create: `src-tauri/src/process/mod.rs`
- Create: `src-tauri/src/process/discovery.rs`
- Create: `src-tauri/src/process/launcher.rs`
- Modify: `src-tauri/src/models.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Produces `process::inspect_running_codex() -> CodexStatus`.
- Produces `process::launch_codex(path: &Path, port: u16) -> Result<(), CommandError>`.
- Consumes only an executable path supplied by discovery or the user; no process-kill API exists.

- [ ] **Step 1: Write failing parser tests**

```rust
#[test]
fn parses_equals_style_debug_port() {
    assert_eq!(parse_debug_port("Codex.exe --remote-debugging-port=43123"), Some(43123));
}

#[test]
fn parses_spaced_style_debug_port() {
    assert_eq!(parse_debug_port("Codex.exe --remote-debugging-port 43124"), Some(43124));
}

#[test]
fn rejects_invalid_port() {
    assert_eq!(parse_debug_port("Codex.exe --remote-debugging-port=0"), None);
}
```

- [ ] **Step 2: Run parser tests and observe failure**

Run: `cargo test process::discovery::tests`
Expected: compilation failure because the parser module is absent.

- [ ] **Step 3: Implement exact process safety behavior**

Implement case-insensitive exact filename matching for `Codex.exe`, parse only positive `u16` values, and return `RunningWithoutDebugPort` rather than launching another instance. Use `std::process::Command` only to start a not-running Codex process with `--remote-debugging-port=<port>`; do not add `taskkill`, `TerminateProcess`, shell execution, or elevation code.

- [ ] **Step 4: Run focused tests and compiler check**

Run: `cargo test process::discovery::tests && cargo check`
Expected: both commands exit with status 0.

- [ ] **Step 5: Record verification result in this plan**

Mark this task only after the commands in Step 4 succeed.

### Task 3: Implement strict-loopback endpoint discovery and CDP JSON-RPC pairing

**Files:**
- Create: `src-tauri/src/cdp/mod.rs`
- Create: `src-tauri/src/cdp/local_endpoint.rs`
- Create: `src-tauri/src/cdp/client.rs`
- Create: `src-tauri/src/cdp/targets.rs`
- Modify: `src-tauri/src/error.rs`
- Modify: `src-tauri/src/models.rs`

**Interfaces:**
- Produces `validate_loopback_ws_url(raw: &str, port: u16) -> Result<Url, CommandError>`.
- Produces `CdpClient::connect(endpoint: Url)`, `CdpClient::call(method, params)`, and `discover_page_targets(port)`.
- `CdpClient::call` assigns monotonic IDs and resolves exactly the matching response; notifications are ignored safely.

- [ ] **Step 1: Write failing endpoint validation tests**

```rust
#[test]
fn accepts_matching_loopback_websocket() {
    assert!(validate_loopback_ws_url("ws://127.0.0.1:43123/devtools/page/1", 43123).is_ok());
}

#[test]
fn rejects_non_loopback_websocket() {
    assert!(validate_loopback_ws_url("ws://192.168.1.10:43123/devtools/page/1", 43123).is_err());
}

#[test]
fn rejects_wrong_port() {
    assert!(validate_loopback_ws_url("ws://127.0.0.1:43124/devtools/page/1", 43123).is_err());
}
```

- [ ] **Step 2: Run the endpoint tests and observe failure**

Run: `cargo test cdp::local_endpoint::tests`
Expected: compilation failure because the CDP modules are absent.

- [ ] **Step 3: Implement local-only discovery and request matching**

Use `tokio::net::TcpStream::connect((Ipv4Addr::LOCALHOST, port))` to issue `GET /json/list HTTP/1.1`, require a successful HTTP response, and deserialize JSON through `serde_json`. Validate every discovered `webSocketDebuggerUrl` before connecting. Split `WebSocketStream` into writer/reader tasks, use `AtomicU64` for IDs, map each ID to a oneshot sender, and fail all pending requests if the reader exits.

- [ ] **Step 4: Run focused tests and compiler check**

Run: `cargo test cdp::local_endpoint::tests && cargo check`
Expected: both commands exit with status 0.

- [ ] **Step 5: Record verification result in this plan**

Mark this task only after the commands in Step 4 succeed.

### Task 4: Add original injection, verification, restoration, and reconnection lifecycle

**Files:**
- Create: `src-tauri/src/injection/mod.rs`
- Create: `src-tauri/src/injection/scripts.rs`
- Create: `src-tauri/src/injection/theme.rs`
- Create: `src-tauri/src/injection/registry.rs`
- Create: `src-tauri/src/cdp/reconnect.rs`
- Create: `src-tauri/src/app_state.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Produces `AppState::apply_theme`, `AppState::verify_theme`, `AppState::restore_theme`, and `AppState::start_reconnector`.
- Injection script accepts a serialized `ThemePayload`, creates `#codeskin-runtime-style`, and writes `data-codeskin-theme-id`.
- Restore removes only CodeSkin-owned style/data attributes and `Page.addScriptToEvaluateOnNewDocument` registrations known to this process.

- [ ] **Step 1: Write failing injection serialization and script-ownership tests**

```rust
#[test]
fn install_script_uses_only_codeskin_owned_markers() {
    assert!(INSTALL_SCRIPT.contains("codeskin-runtime-style"));
    assert!(RESTORE_SCRIPT.contains("codeskin-runtime-style"));
    assert!(!INSTALL_SCRIPT.contains("app.asar"));
}

#[test]
fn payload_json_escapes_theme_values() {
    let payload = ThemePayload::from_theme(&Theme::builtin()[0]);
    assert!(serde_json::to_string(&payload).is_ok());
}
```

- [ ] **Step 2: Run injection tests and observe failure**

Run: `cargo test injection::tests`
Expected: compilation failure because injection modules are absent.

- [ ] **Step 3: Implement injection lifecycle**

Create newly-authored JavaScript strings that build CSS variables and a style tag, register the exact install source with `Page.addScriptToEvaluateOnNewDocument`, run `Runtime.evaluate` immediately for already-loaded pages, verify the marker and computed custom property, and restore by removing the marker/style tag plus stored script identifiers. On disconnect, use a bounded retry loop that re-discovers only the known loopback port and never panics on request failure.

- [ ] **Step 4: Run tests and full Rust compilation**

Run: `cargo test injection::tests && cargo check`
Expected: both commands exit with status 0.

- [ ] **Step 5: Record verification result in this plan**

Mark this task only after the commands in Step 4 succeed.

### Task 5: Add persistent theme/background storage, tray behavior, and functional React UI

**Files:**
- Create: `src-tauri/src/storage/mod.rs`
- Create: `src-tauri/src/storage/themes.rs`
- Create: `src-tauri/src/storage/backgrounds.rs`
- Create: `src-tauri/src/tray.rs`
- Modify: `src-tauri/src/app_state.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/App.tsx`
- Modify: `src/App.css`
- Modify: `src/api.ts`
- Modify: `src/types.ts`

**Interfaces:**
- Produces `ThemeStore::load/save` and `BackgroundStore::import_bytes`.
- Background import rejects values greater than 12 MiB or decoded dimensions greater than 8192 by 8192.
- Produces tray menu IDs `show`, `restore`, and `quit`; quit attempts restore and then exits.

- [ ] **Step 1: Write failing storage tests**

```rust
#[test]
fn rejects_large_background_input() {
    let too_large = vec![0_u8; 12 * 1024 * 1024 + 1];
    assert!(validate_background_bytes(&too_large).is_err());
}
```

- [ ] **Step 2: Run storage tests and observe failure**

Run: `cargo test storage::backgrounds::tests`
Expected: compilation failure because storage modules are absent.

- [ ] **Step 3: Implement persistence, UI flow, and tray**

Store selected theme as JSON under Tauri app data, import browser-selected image bytes after validation, add frontend status/Apply/Verify/Restore controls, surface errors as text, hide instead of exit on main-window close, and make tray actions show, restore, or gracefully quit. The UI text must identify CodeSkin as non-official.

- [ ] **Step 4: Run Rust tests, Rust check, and frontend build**

Run: `cargo test && cargo check && npm run build`
Expected: all commands exit with status 0.

- [ ] **Step 5: Record verification result in this plan**

Mark this task only after the commands in Step 4 succeed.

### Task 6: Package and validate manually against Codex

**Files:**
- Modify: `README.md`
- Modify: `docs/superpowers/plans/2026-07-17-codeskin-runtime-theme-tool.md`

**Interfaces:**
- Documents the supported behavior and the manual checklist; no new runtime APIs.

- [ ] **Step 1: Add the user-facing operating constraints to README**

Document that Codex must be closed before CodeSkin can relaunch it with a local debug port, that CodeSkin never edits Codex files, and that it is non-official.

- [ ] **Step 2: Build the release executable**

Run: `cargo build --release`
Expected: exit status 0 and `target/release/codeskin.exe` exists.

- [ ] **Step 3: Perform manual Codex checks when an installed Codex.exe is available**

Run the application and record each actual outcome:

1. Codex closed: CodeSkin launches it with `--remote-debugging-port=<selected-port>` and connects.
2. Codex already running with the port: CodeSkin detects the port and does not launch another process.
3. Apply a theme: an observable color changes.
4. Refresh and open a new Codex window: the theme reappears.
5. Verify: every target returns an active injection marker.
6. Restore: the marker/style are removed and the Codex appearance returns to its unmodified state.
7. Restart Codex: CodeSkin retries without crash or deadlock and reconnects after the replacement process starts.

If Codex launch arguments, port behavior, or authentication differ from the specification, stop implementation and report the observed behavior; do not invent a workaround.

- [ ] **Step 4: Re-run complete automated verification after documentation edits**

Run: `cargo test && cargo check && npm run build && cargo build --release`
Expected: all commands exit with status 0.

- [ ] **Step 5: Record actual results and any unavailable runtime validation**

Mark only the checks that were actually executed. State explicitly if Codex.exe was not installed or a specified behavior could not be observed.