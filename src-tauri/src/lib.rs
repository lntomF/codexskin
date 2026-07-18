mod app_state;
mod cdp;
mod commands;
mod error;
mod injection;
mod models;
mod process;
mod storage;
mod tray;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(app_state::AppState::new())
        .setup(|app| {
            tray::build(app)?;

            // Windows may keep the currently active application in the foreground when
            // CodeSkin is launched externally. Ensure the first-run window is visible
            // and asks for focus; later close requests still minimize to the tray.
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
            commands::load_theme_library,
            commands::connect_or_start_codex,
            commands::apply_theme,
            commands::import_wallpaper_theme,
            commands::rename_theme,
            commands::verify_theme,
            commands::restore_theme
        ])
        .run(tauri::generate_context!())
        .expect("error while running CodeSkin");
}
