use crate::{app_state::AppState, commands};
use std::sync::Arc;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Manager,
};

const TRAY_ID: &str = "codeskin-tray";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrayMenuAction {
    Show,
    Restore,
    Exit,
}

pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let menu = build_menu(app)?;
    let icon = tray_icon(app.default_window_icon())?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .menu(&menu)
        .tooltip("CodeSkin — 非官方 Codex 背景工具")
        .on_menu_event(on_menu_event)
        .build(app)?;
    Ok(())
}

fn tray_icon(
    icon: Option<&tauri::image::Image<'_>>,
) -> tauri::Result<tauri::image::Image<'static>> {
    match icon {
        Some(icon) => Ok(icon.clone().to_owned()),
        None => Err(tauri::Error::AssetNotFound(
            "application icon for tray".into(),
        )),
    }
}

fn build_menu(manager: &impl Manager<tauri::Wry>) -> tauri::Result<Menu<tauri::Wry>> {
    let show = MenuItem::with_id(manager, "show", "显示 CodeSkin", true, None::<&str>)?;
    let restore = MenuItem::with_id(
        manager,
        "restore",
        "恢复 Codex 原始外观",
        true,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(manager, "quit", "退出", true, None::<&str>)?;
    Menu::with_items(manager, &[&show, &restore, &quit])
}

fn on_menu_event(app: &AppHandle, event: tauri::menu::MenuEvent) {
    match menu_action(event.id.as_ref()) {
        Some(TrayMenuAction::Show) => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        Some(TrayMenuAction::Restore) => {
            let state = app.state::<Arc<AppState>>().inner().clone();
            tauri::async_runtime::spawn(async move {
                let _ = commands::restore_original_appearance_inner(&state).await;
            });
        }
        Some(TrayMenuAction::Exit) => {
            app.exit(0);
        }
        None => {}
    }
}

fn menu_action(id: &str) -> Option<TrayMenuAction> {
    match id {
        "show" => Some(TrayMenuAction::Show),
        "restore" => Some(TrayMenuAction::Restore),
        "quit" => Some(TrayMenuAction::Exit),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{menu_action, tray_icon, TrayMenuAction};

    #[test]
    fn packaged_icon_is_available_for_the_tray() {
        let context: tauri::Context<tauri::Wry> = tauri::generate_context!();
        let icon = tray_icon(
            context
                .tray_icon()
                .or_else(|| context.default_window_icon()),
        )
        .expect("the packaged application icon should be available for the tray");

        assert!(icon.width() > 0);
        assert!(icon.height() > 0);
        assert!(!icon.rgba().is_empty());
    }
    #[test]
    fn quit_menu_action_exits_without_restoring_the_active_theme() {
        assert_eq!(menu_action("quit"), Some(TrayMenuAction::Exit));
    }

    #[test]
    fn restore_menu_action_remains_the_only_explicit_restore_path() {
        assert_eq!(menu_action("restore"), Some(TrayMenuAction::Restore));
    }
}
