mod app_state;
mod cdp;
mod commands;
mod error;
mod injection;
mod models;
mod process;
mod storage;
mod tray;

use std::fmt::Display;
use tauri::Manager;

/// Temporary, opt-in runtime diagnostics for persistence and CDP recovery.
/// Kept inert unless explicitly enabled while investigating an installation.
pub(crate) fn diagnostic(message: impl Display) {
    if std::env::var_os("CODESKIN_DIAGNOSTICS").as_deref() == Some(std::ffi::OsStr::new("1")) {
        let line = format!("[codeskin-diagnostic] {message}");
        eprintln!("{line}");
        let path = std::env::var_os("LOCALAPPDATA")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("CodeSkin")
            .join("diagnostics.log");
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            use std::io::Write;
            let _ = writeln!(file, "{line}");
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_state = app_state::AppState::new();
    let app_state_for_setup = std::sync::Arc::clone(&app_state);
    tauri::Builder::default()
        .manage(app_state)
        .setup(move |app| {
            tray::build(app.handle())?;
            let recovery_state = std::sync::Arc::clone(&app_state_for_setup);
            tauri::async_runtime::spawn(async move {
                recovery_state.restore_persisted_theme_on_startup().await;
            });
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::inspect_codex_status,
            commands::load_background_library,
            commands::connect_or_start_codex,
            commands::apply_background,
            commands::import_background,
            commands::delete_background,
            commands::verify_injection,
            commands::restore_original_appearance
        ])
        .run(tauri::generate_context!())
        .expect("error while running CodeSkin");
}
