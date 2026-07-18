use crate::{
    error::CommandError,
    models::{Theme, ThemeColors, ThemeLayers, ThemeLibrary, ThemeSource},
};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

pub use crate::models::THEME_LIBRARY_VERSION;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistedSettings {
    pub selected_theme_id: Option<String>,
    pub background_image: Option<String>,
}

pub fn load_theme_library() -> Result<ThemeLibrary, CommandError> {
    let path = theme_library_path()?;
    let mut library = match fs::read(&path) {
        Ok(bytes) => deserialize_theme_library(&bytes)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            load_or_migrate_legacy_settings()?
        }
        Err(error) => {
            return Err(CommandError::new(
                "background_library_read_failed",
                error.to_string(),
            ))
        }
    };
    let changed = normalize_background_library(&mut library);
    if changed {
        save_theme_library(&library)?;
    }
    Ok(library)
}

pub fn save_theme_library(library: &ThemeLibrary) -> Result<(), CommandError> {
    validate_theme_library_version(library)?;
    let path = theme_library_path()?;
    let serialized = serde_json::to_vec_pretty(library).map_err(|error| {
        CommandError::new("background_library_serialize_failed", error.to_string())
    })?;
    fs::write(path, serialized)
        .map_err(|error| CommandError::new("background_library_write_failed", error.to_string()))
}

pub(crate) fn deserialize_theme_library(bytes: &[u8]) -> Result<ThemeLibrary, CommandError> {
    let library: ThemeLibrary = serde_json::from_slice(bytes)
        .map_err(|error| CommandError::new("background_library_read_failed", error.to_string()))?;
    validate_theme_library_version(&library)?;
    Ok(library)
}

fn load_or_migrate_legacy_settings() -> Result<ThemeLibrary, CommandError> {
    match fs::read(settings_path()?) {
        Ok(bytes) => {
            let mut library = migrate_legacy_settings(&bytes)?;
            normalize_background_library(&mut library);
            save_theme_library(&library)?;
            Ok(library)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(ThemeLibrary::empty()),
        Err(error) => Err(CommandError::new("settings_read_failed", error.to_string())),
    }
}

pub(crate) fn migrate_legacy_settings(bytes: &[u8]) -> Result<ThemeLibrary, CommandError> {
    let settings: PersistedSettings = serde_json::from_slice(bytes)
        .map_err(|error| CommandError::new("settings_read_failed", error.to_string()))?;
    let Some(image) = settings.background_image else {
        return Ok(ThemeLibrary::empty());
    };
    let id = "legacy-background".to_owned();
    Ok(ThemeLibrary {
        version: THEME_LIBRARY_VERSION,
        selected_theme_id: Some(id.clone()),
        themes: vec![Theme {
            id,
            name: "已迁移背景".into(),
            description: "从旧版 CodeSkin 设置迁移。".into(),
            colors: ThemeColors {
                accent: "#8B9DFF".into(),
                secondary: "#8B9DFF".into(),
                background: "#11131F".into(),
                surface: "#1A1E30".into(),
                foreground: "#EDF0FF".into(),
                muted: "#AAB2CE".into(),
            },
            background_image: Some(image.clone()),
            source_image: Some(image),
            source: ThemeSource::Wallpaper,
            layers: ThemeLayers::wallpaper(),
            contrast: None,
        }],
    })
}

fn normalize_background_library(library: &mut ThemeLibrary) -> bool {
    let original = library.clone();
    library.version = THEME_LIBRARY_VERSION;
    library.themes.retain(|background| {
        background.source == ThemeSource::Wallpaper && background.background_image.is_some()
    });
    if let Some(selected) = library.selected_theme_id.as_ref() {
        if !library
            .themes
            .iter()
            .any(|background| &background.id == selected)
        {
            library.selected_theme_id = None;
        }
    }
    *library != original
}

fn validate_theme_library_version(library: &ThemeLibrary) -> Result<(), CommandError> {
    if library.version > THEME_LIBRARY_VERSION {
        return Err(CommandError::new(
            "background_library_version_unsupported",
            format!(
                "背景库版本 {} 高于当前支持的版本 {}。",
                library.version, THEME_LIBRARY_VERSION
            ),
        ));
    }
    Ok(())
}

pub(crate) fn app_data_dir() -> Result<PathBuf, CommandError> {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let directory = base.join("CodeSkin");
    fs::create_dir_all(&directory)
        .map_err(|error| CommandError::new("app_data_create_failed", error.to_string()))?;
    Ok(directory)
}

pub(crate) fn theme_library_path() -> Result<PathBuf, CommandError> {
    Ok(app_data_dir()?.join("themes.json"))
}
fn settings_path() -> Result<PathBuf, CommandError> {
    Ok(app_data_dir()?.join("settings.json"))
}

#[cfg(test)]
mod tests {
    use super::{deserialize_theme_library, migrate_legacy_settings, THEME_LIBRARY_VERSION};

    #[test]
    fn reads_v1_themes_key_but_writes_as_backgrounds() {
        let v1 = br#"{"version":1,"selectedThemeId":"midnight-ink","themes":[]}"#;
        let library = deserialize_theme_library(v1).unwrap();
        assert_eq!(library.version, 1);
        let serialized = serde_json::to_string(&library).unwrap();
        assert!(serialized.contains("backgrounds"));
        assert!(serialized.contains("selectedBackgroundId"));
    }

    #[test]
    fn migrates_a_legacy_wallpaper_without_restoring_builtin_themes() {
        let library = migrate_legacy_settings(
            br#"{"selectedThemeId":"midnight-ink","backgroundImage":"file:///C:/old.png"}"#,
        )
        .unwrap();
        assert_eq!(library.version, THEME_LIBRARY_VERSION);
        assert_eq!(library.themes.len(), 1);
        assert_eq!(
            library.themes[0].background_image.as_deref(),
            Some("file:///C:/old.png")
        );
    }
}
