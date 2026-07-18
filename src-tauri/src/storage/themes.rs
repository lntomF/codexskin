use crate::{
    error::CommandError,
    models::{Theme, ThemeLibrary, ThemeSource},
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
    let library_path = theme_library_path()?;
    match fs::read(&library_path) {
        Ok(bytes) => deserialize_theme_library(&bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            load_or_migrate_legacy_settings()
        }
        Err(error) => Err(CommandError::new(
            "theme_library_read_failed",
            error.to_string(),
        )),
    }
}

pub fn save_theme_library(library: &ThemeLibrary) -> Result<(), CommandError> {
    validate_theme_library_version(library)?;
    let path = theme_library_path()?;
    let serialized = serde_json::to_vec_pretty(library)
        .map_err(|error| CommandError::new("theme_library_serialize_failed", error.to_string()))?;
    fs::write(path, serialized)
        .map_err(|error| CommandError::new("theme_library_write_failed", error.to_string()))
}

pub(crate) fn deserialize_theme_library(bytes: &[u8]) -> Result<ThemeLibrary, CommandError> {
    let library: ThemeLibrary = serde_json::from_slice(bytes)
        .map_err(|error| CommandError::new("theme_library_read_failed", error.to_string()))?;
    validate_theme_library_version(&library)?;
    Ok(library)
}

pub(crate) fn migrate_legacy_settings(bytes: &[u8]) -> Result<ThemeLibrary, CommandError> {
    let settings: PersistedSettings = serde_json::from_slice(bytes)
        .map_err(|error| CommandError::new("settings_read_failed", error.to_string()))?;
    Ok(theme_library_from_legacy_settings(settings))
}

fn load_or_migrate_legacy_settings() -> Result<ThemeLibrary, CommandError> {
    let legacy_path = settings_path()?;
    match fs::read(legacy_path) {
        Ok(bytes) => {
            let library = migrate_legacy_settings(&bytes)?;
            save_theme_library(&library)?;
            Ok(library)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(ThemeLibrary::with_builtin_themes())
        }
        Err(error) => Err(CommandError::new("settings_read_failed", error.to_string())),
    }
}

fn validate_theme_library_version(library: &ThemeLibrary) -> Result<(), CommandError> {
    if library.version > THEME_LIBRARY_VERSION {
        return Err(CommandError::new(
            "theme_library_version_unsupported",
            format!(
                "主题库版本 {} 高于当前支持的版本 {}。",
                library.version, THEME_LIBRARY_VERSION
            ),
        ));
    }
    Ok(())
}

fn theme_library_from_legacy_settings(settings: PersistedSettings) -> ThemeLibrary {
    let mut library = ThemeLibrary::with_builtin_themes();
    let Some(selected_builtin_id) = settings.selected_theme_id else {
        return library;
    };

    let Some(selected_builtin) = library
        .themes
        .iter()
        .find(|theme| theme.id == selected_builtin_id && theme.source == ThemeSource::Builtin)
        .cloned()
    else {
        return library;
    };

    match settings.background_image {
        Some(background_image) => {
            let wallpaper_id = migrated_wallpaper_id(&selected_builtin.id, &library.themes);
            let mut wallpaper_theme = selected_builtin;
            wallpaper_theme.id = wallpaper_id.clone();
            wallpaper_theme.name = format!("{} Wallpaper", wallpaper_theme.name);
            wallpaper_theme.description = format!(
                "{}（从旧版 CodeSkin 设置迁移）",
                wallpaper_theme.description
            );
            wallpaper_theme.source = ThemeSource::Wallpaper;
            wallpaper_theme.background_image = Some(background_image);
            library.themes.push(wallpaper_theme);
            library.selected_theme_id = Some(wallpaper_id);
        }
        None => library.selected_theme_id = Some(selected_builtin_id),
    }

    library
}

fn migrated_wallpaper_id(builtin_id: &str, themes: &[Theme]) -> String {
    let base = format!("{builtin_id}-wallpaper");
    if !themes.iter().any(|theme| theme.id == base) {
        return base;
    }

    let mut suffix = 2;
    loop {
        let candidate = format!("{base}-{suffix}");
        if !themes.iter().any(|theme| theme.id == candidate) {
            return candidate;
        }
        suffix += 1;
    }
}

// Compatibility facade for the current command layer. New callers should use the
// versioned ThemeLibrary API above; these accessors keep existing callers on the
// themes.json persistence path until they are migrated in the next task.
pub fn load_settings() -> Result<PersistedSettings, CommandError> {
    let library = load_theme_library()?;
    let selected_theme = library
        .selected_theme_id
        .as_deref()
        .and_then(|id| library.themes.iter().find(|theme| theme.id == id));

    Ok(PersistedSettings {
        selected_theme_id: library.selected_theme_id,
        background_image: selected_theme.and_then(|theme| theme.background_image.clone()),
    })
}

pub fn save_settings(settings: &PersistedSettings) -> Result<(), CommandError> {
    let mut library = load_theme_library()?;
    let selected_builtin = settings.selected_theme_id.as_deref().and_then(|id| {
        library
            .themes
            .iter()
            .find(|theme| theme.id == id && theme.source == ThemeSource::Builtin)
            .cloned()
    });

    match (selected_builtin, settings.background_image.clone()) {
        (Some(builtin), Some(background_image)) => {
            let wallpaper_id = migrated_wallpaper_id(&builtin.id, &library.themes);
            let mut wallpaper = builtin;
            wallpaper.id = wallpaper_id.clone();
            wallpaper.name = format!("{} Wallpaper", wallpaper.name);
            wallpaper.source = ThemeSource::Wallpaper;
            wallpaper.background_image = Some(background_image);
            library.themes.retain(|theme| theme.id != wallpaper_id);
            library.themes.push(wallpaper);
            library.selected_theme_id = Some(wallpaper_id);
        }
        (Some(builtin), None) => library.selected_theme_id = Some(builtin.id),
        (None, _) => library.selected_theme_id = settings.selected_theme_id.clone(),
    }

    save_theme_library(&library)
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
    use crate::models::ThemeSource;

    #[test]
    fn migrates_legacy_selected_theme_and_background() {
        let legacy = br#"{"selectedThemeId":"midnight-ink","backgroundImage":"C:/CodeSkin/backgrounds/old.png"}"#;
        let library = migrate_legacy_settings(legacy).expect("legacy settings migrate");
        let selected = library
            .selected_theme_id
            .as_deref()
            .expect("selected migrated theme");
        let theme = library
            .themes
            .iter()
            .find(|theme| theme.id == selected)
            .expect("selected theme");

        assert_eq!(library.version, THEME_LIBRARY_VERSION);
        assert_eq!(theme.source, ThemeSource::Wallpaper);
        assert_eq!(
            theme.background_image.as_deref(),
            Some("C:/CodeSkin/backgrounds/old.png")
        );
    }

    #[test]
    fn rejects_a_theme_library_from_a_newer_version() {
        let library = br#"{"version":2,"selectedThemeId":null,"themes":[]}"#;
        let error = deserialize_theme_library(library).expect_err("newer version must fail");
        assert_eq!(error.code, "theme_library_version_unsupported");
    }
}
