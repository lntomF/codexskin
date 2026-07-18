use crate::{
    app_state::AppState,
    commands,
    error::CommandError,
    models::{Theme, ThemeLibrary},
    storage,
};
use std::{io, sync::Arc};
use tauri::{
    menu::{Menu, MenuItem, Submenu},
    tray::TrayIconBuilder,
    App, AppHandle, Manager, Wry,
};

const TRAY_ID: &str = "codeskin-tray";
const THEME_EVENT_PREFIX: &str = "theme:";

pub fn build(app: &App) -> tauri::Result<()> {
    let library = storage::load_theme_library().map_err(command_error_to_tauri)?;
    let menu = build_menu(app, &library)?;

    TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("CodeSkin — Codex 非官方换肤工具")
        .menu(&menu)
        .on_menu_event(on_menu_event)
        .build(app)?;

    sync_theme_menu(&app.handle()).map_err(command_error_to_tauri)
}

pub fn build_menu(app: &App, library: &ThemeLibrary) -> tauri::Result<Menu<Wry>> {
    build_menu_for_manager(app, library)
}

fn build_menu_for_manager<M: Manager<Wry>>(
    manager: &M,
    library: &ThemeLibrary,
) -> tauri::Result<Menu<Wry>> {
    let show = MenuItem::with_id(manager, "show", "显示 CodeSkin", true, None::<&str>)?;
    let restore = MenuItem::with_id(
        manager,
        "restore",
        "恢复 Codex 原始外观",
        true,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(manager, "quit", "退出", true, None::<&str>)?;
    let themes = Submenu::new(manager, "主题", true)?;

    if library.themes.is_empty() {
        let empty = MenuItem::with_id(
            manager,
            "themes-empty",
            "没有已保存的主题",
            false,
            None::<&str>,
        )?;
        themes.append(&empty)?;
    } else {
        for theme in &library.themes {
            let item = MenuItem::with_id(
                manager,
                theme_event_id(&theme.id).map_err(command_error_to_tauri)?,
                theme_menu_label(theme, library.selected_theme_id.as_deref()),
                true,
                None::<&str>,
            )?;
            themes.append(&item)?;
        }
    }

    Menu::with_items(manager, &[&show, &themes, &restore, &quit])
}

pub fn sync_theme_menu(app: &AppHandle) -> Result<(), CommandError> {
    let library = storage::load_theme_library()?;
    let menu = build_menu_for_manager(app, &library)
        .map_err(|error| CommandError::new("tray_menu_build_failed", error.to_string()))?;
    let tray = app
        .tray_by_id(TRAY_ID)
        .ok_or_else(|| CommandError::new("tray_not_found", "找不到 CodeSkin 托盘图标。"))?;

    tray.set_menu(Some(menu))
        .map_err(|error| CommandError::new("tray_menu_update_failed", error.to_string()))
}

pub(crate) fn theme_menu_label(theme: &Theme, selected_id: Option<&str>) -> String {
    if selected_id == Some(theme.id.as_str()) {
        format!("✓ {}", theme.name)
    } else {
        theme.name.clone()
    }
}

pub(crate) fn theme_event_id(theme_id: &str) -> Result<String, CommandError> {
    if theme_id.is_empty() || theme_id.contains(':') {
        return Err(CommandError::new(
            "theme_id_invalid",
            "主题 ID 不能为空且不能包含冒号。",
        ));
    }

    Ok(format!("{THEME_EVENT_PREFIX}{theme_id}"))
}

fn on_menu_event(app: &AppHandle, event: tauri::menu::MenuEvent) {
    let event_id = event.id.as_ref();
    if let Some(theme_id) = event_id.strip_prefix(THEME_EVENT_PREFIX) {
        let is_valid_theme_event = matches!(
            theme_event_id(theme_id),
            Ok(expected_event_id) if expected_event_id == event_id
        );
        if !is_valid_theme_event {
            return;
        }

        let state = app.state::<Arc<AppState>>().inner().clone();
        let app_handle = app.clone();
        let theme_id = theme_id.to_owned();
        tauri::async_runtime::spawn(async move {
            if commands::apply_saved_theme_by_id(&theme_id, &state)
                .await
                .is_ok()
            {
                let _ = sync_theme_menu(&app_handle);
            }
        });
        return;
    }

    match event_id {
        "show" => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        "restore" => {
            let state = app.state::<Arc<AppState>>().inner().clone();
            tauri::async_runtime::spawn(async move {
                let _ = state.restore_theme().await;
            });
        }
        "quit" => {
            let state = app.state::<Arc<AppState>>().inner().clone();
            let app_handle = app.clone();
            tauri::async_runtime::spawn(async move {
                let _ = state.restore_theme().await;
                app_handle.exit(0);
            });
        }
        _ => {}
    }
}

fn command_error_to_tauri(error: CommandError) -> tauri::Error {
    io::Error::new(io::ErrorKind::InvalidInput, error.to_string()).into()
}

#[cfg(test)]
mod tests {
    use super::{theme_event_id, theme_menu_label};
    use crate::models::Theme;

    #[test]
    fn marks_only_the_selected_theme_menu_label() {
        let theme = Theme::builtin().into_iter().next().expect("built-in theme");

        assert_eq!(
            theme_menu_label(&theme, Some(theme.id.as_str())),
            format!("✓ {}", theme.name)
        );
        assert_eq!(theme_menu_label(&theme, Some("another-theme")), theme.name);
    }

    #[test]
    fn creates_reversible_event_ids_for_any_valid_theme_source() {
        assert_eq!(
            theme_event_id("midnight-ink").expect("built-in theme ID is valid"),
            "theme:midnight-ink"
        );
        assert_eq!(
            theme_event_id("wallpaper-123").expect("wallpaper theme ID is valid"),
            "theme:wallpaper-123"
        );
    }

    #[test]
    fn rejects_empty_or_ambiguous_theme_event_ids() {
        for theme_id in ["", "custom:theme"] {
            let error = theme_event_id(theme_id).expect_err("unsafe theme ID must be rejected");
            assert_eq!(error.code, "theme_id_invalid");
        }
    }
}
