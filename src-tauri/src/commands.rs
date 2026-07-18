use crate::{
    app_state::AppState,
    error::CommandError,
    models::{CodexStatus, ThemeLibrary, ThemeSource, VerifyResult},
    process, storage, tray,
};
use std::sync::Arc;
use tauri::{AppHandle, State};

#[tauri::command]
pub fn inspect_codex_status() -> CodexStatus {
    process::inspect_running_codex()
}

#[tauri::command]
pub fn load_theme_library() -> Result<ThemeLibrary, CommandError> {
    storage::load_theme_library()
}

#[tauri::command]
pub async fn connect_or_start_codex(
    state: State<'_, Arc<AppState>>,
) -> Result<CodexStatus, CommandError> {
    state.connect_or_start_codex().await
}

#[tauri::command]
pub async fn apply_theme(
    theme_id: String,
    state: State<'_, Arc<AppState>>,
    app: AppHandle,
) -> Result<VerifyResult, CommandError> {
    let result = apply_saved_theme_by_id(&theme_id, state.inner()).await?;
    tray::sync_theme_menu(&app)?;
    Ok(result)
}

pub(crate) async fn apply_saved_theme_by_id(
    theme_id: &str,
    state: &Arc<AppState>,
) -> Result<VerifyResult, CommandError> {
    let mut library = storage::load_theme_library()?;
    let theme = library
        .themes
        .iter()
        .find(|theme| theme.id == theme_id)
        .cloned()
        .ok_or_else(|| CommandError::new("theme_not_found", "找不到请求的主题。"))?;

    let result = state.apply_saved_theme(theme).await?;
    library.selected_theme_id = Some(theme_id.to_owned());
    storage::save_theme_library(&library)?;
    state.start_reconnector();
    Ok(result)
}

#[tauri::command]
pub async fn import_wallpaper_theme(
    bytes: Vec<u8>,
    display_name: String,
    state: State<'_, Arc<AppState>>,
    app: AppHandle,
) -> Result<ThemeLibrary, CommandError> {
    let display_name = display_name.trim();
    let mut theme = storage::import_wallpaper_theme(
        &bytes,
        if display_name.is_empty() {
            "Wallpaper"
        } else {
            display_name
        },
    )?;
    let mut library = storage::load_theme_library()?;
    theme.id = unique_theme_id(&theme.id, &library);
    library.selected_theme_id = Some(theme.id.clone());
    library.themes.push(theme.clone());
    storage::save_theme_library(&library)?;

    state.inner().remember_active_theme(theme).await;
    state.inner().start_reconnector();
    tray::sync_theme_menu(&app)?;
    Ok(library)
}

#[tauri::command]
pub fn rename_theme(
    theme_id: String,
    name: String,
    app: AppHandle,
) -> Result<ThemeLibrary, CommandError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(CommandError::new(
            "theme_name_invalid",
            "主题名称不能为空。",
        ));
    }

    let mut library = storage::load_theme_library()?;
    let theme = library
        .themes
        .iter_mut()
        .find(|theme| theme.id == theme_id)
        .ok_or_else(|| CommandError::new("theme_not_found", "找不到请求的主题。"))?;
    if theme.source != ThemeSource::Wallpaper {
        return Err(CommandError::new(
            "theme_rename_not_allowed",
            "内置主题不能重命名。",
        ));
    }

    theme.name = name.to_owned();
    storage::save_theme_library(&library)?;
    tray::sync_theme_menu(&app)?;
    Ok(library)
}

#[tauri::command]
pub async fn verify_theme(state: State<'_, Arc<AppState>>) -> Result<VerifyResult, CommandError> {
    state.verify_theme().await
}

#[tauri::command]
pub async fn restore_theme(state: State<'_, Arc<AppState>>) -> Result<VerifyResult, CommandError> {
    state.restore_theme().await
}

fn unique_theme_id(base_id: &str, library: &ThemeLibrary) -> String {
    if !library.themes.iter().any(|theme| theme.id == base_id) {
        return base_id.to_owned();
    }

    let mut suffix = 2_u32;
    loop {
        let candidate = format!("{base_id}-{suffix}");
        if !library.themes.iter().any(|theme| theme.id == candidate) {
            return candidate;
        }
        suffix = suffix.saturating_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::unique_theme_id;
    use crate::models::ThemeLibrary;

    #[test]
    fn resolves_wallpaper_theme_id_collisions_with_incrementing_suffixes() {
        let mut library = ThemeLibrary::with_builtin_themes();
        library.themes.push(crate::models::Theme {
            id: "wallpaper-sunset".into(),
            name: "Sunset".into(),
            description: "test".into(),
            colors: crate::models::ThemeColors {
                accent: "#000000".into(),
                background: "#000000".into(),
                surface: "#000000".into(),
                foreground: "#000000".into(),
                muted: "#000000".into(),
            },
            background_image: Some("C:/example.jpg".into()),
            source: crate::models::ThemeSource::Wallpaper,
            layers: crate::models::ThemeLayers::builtin(),
        });
        library.themes.push(crate::models::Theme {
            id: "wallpaper-sunset-2".into(),
            name: "Sunset 2".into(),
            description: "test".into(),
            colors: crate::models::ThemeColors {
                accent: "#000000".into(),
                background: "#000000".into(),
                surface: "#000000".into(),
                foreground: "#000000".into(),
                muted: "#000000".into(),
            },
            background_image: Some("C:/example-2.jpg".into()),
            source: crate::models::ThemeSource::Wallpaper,
            layers: crate::models::ThemeLayers::builtin(),
        });

        assert_eq!(
            unique_theme_id("wallpaper-sunset", &library),
            "wallpaper-sunset-3"
        );
        assert_eq!(
            unique_theme_id("wallpaper-river", &library),
            "wallpaper-river"
        );
    }
}
