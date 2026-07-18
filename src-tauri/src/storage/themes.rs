use crate::{
    diagnostic,
    error::CommandError,
    models::{Theme, ThemeColors, ThemeLayers, ThemeLibrary, ThemeSource},
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub use crate::models::THEME_LIBRARY_VERSION;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistedSettings {
    pub selected_theme_id: Option<String>,
    pub background_image: Option<String>,
}

pub fn load_theme_library() -> Result<ThemeLibrary, CommandError> {
    let path = theme_library_path()?;
    let library = match load_theme_library_from_path(&path) {
        Ok(library) => library,
        Err(error) if error.code == "background_library_not_found" => {
            load_or_migrate_legacy_settings()?
        }
        Err(error) => return Err(error),
    };
    diagnostic(format_args!(
        "[storage/load] path={} selectedThemeId={:?} backgrounds={} after_normalize",
        path.display(),
        library.selected_theme_id,
        library.themes.len()
    ));
    Ok(library)
}

pub(crate) fn load_theme_library_from_path(path: &Path) -> Result<ThemeLibrary, CommandError> {
    let bytes = fs::read(path).map_err(|error| {
        let code = if error.kind() == std::io::ErrorKind::NotFound {
            "background_library_not_found"
        } else {
            "background_library_read_failed"
        };
        CommandError::new(code, error.to_string())
    })?;
    let mut library = deserialize_theme_library(&bytes)?;
    if normalize_background_library(&mut library) {
        save_theme_library_to_path(path, &library)?;
    }
    Ok(library)
}

pub fn save_theme_library(library: &ThemeLibrary) -> Result<(), CommandError> {
    let path = theme_library_path()?;
    save_theme_library_to_path(&path, library)
}

pub(crate) fn save_theme_library_to_path(
    path: &Path,
    library: &ThemeLibrary,
) -> Result<(), CommandError> {
    validate_theme_library_version(library)?;
    let serialized = serde_json::to_vec_pretty(library).map_err(|error| {
        CommandError::new("background_library_serialize_failed", error.to_string())
    })?;
    diagnostic(format_args!(
        "[save] requested path={} selectedThemeId={:?} wallpaper={:?} palette={:?}",
        path.display(),
        library.selected_theme_id,
        library
            .selected_theme_id
            .as_ref()
            .and_then(|id| library.themes.iter().find(|theme| &theme.id == id))
            .and_then(|theme| theme.background_image.as_deref()),
        library
            .selected_theme_id
            .as_ref()
            .and_then(|id| library.themes.iter().find(|theme| &theme.id == id))
            .map(|theme| (
                &theme.colors.accent,
                &theme.colors.secondary,
                &theme.colors.background
            ))
    ));
    atomic_write(path, &serialized)?;

    if std::env::var_os("CODESKIN_DIAGNOSTICS").as_deref() == Some(std::ffi::OsStr::new("1")) {
        match fs::read(path).and_then(|bytes| {
            let raw = String::from_utf8_lossy(&bytes);
            let selected_present = library
                .selected_theme_id
                .as_deref()
                .is_some_and(|id| raw.contains(id));
            Ok((bytes, selected_present))
        }) {
            Ok((bytes, selected_present)) => match deserialize_theme_library(&bytes) {
                Ok(on_disk) => diagnostic(format_args!(
                    "[save] verified on disk path={} bytes={} selectedThemeId={:?} selectedIdPresent={} backgrounds={}",
                    path.display(),
                    bytes.len(),
                    on_disk.selected_theme_id,
                    selected_present,
                    on_disk.themes.len()
                )),
                Err(error) => diagnostic(format_args!(
                    "[save] post-write parse failed path={}: {error}",
                    path.display()
                )),
            },
            Err(error) => diagnostic(format_args!(
                "[save] post-write read failed path={}: {error}",
                path.display()
            )),
        }
    }
    Ok(())
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), CommandError> {
    let parent = path.parent().ok_or_else(|| {
        CommandError::new("background_library_write_failed", "主题库路径没有父目录。")
    })?;
    fs::create_dir_all(parent)
        .map_err(|error| CommandError::new("background_library_write_failed", error.to_string()))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            CommandError::new("background_library_write_failed", "主题库路径没有文件名。")
        })?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temporary_path = parent.join(format!(".{file_name}.{}.{}.tmp", std::process::id(), nonce));
    let write_result = (|| -> Result<(), CommandError> {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary_path)
            .map_err(|error| {
                CommandError::new("background_library_write_failed", error.to_string())
            })?;
        file.write_all(bytes).map_err(|error| {
            CommandError::new("background_library_write_failed", error.to_string())
        })?;
        file.sync_all().map_err(|error| {
            CommandError::new("background_library_write_failed", error.to_string())
        })?;
        drop(file);
        fs::rename(&temporary_path, path).map_err(|error| {
            CommandError::new("background_library_write_failed", error.to_string())
        })?;
        Ok(())
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    write_result
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
    use super::{
        deserialize_theme_library, load_theme_library_from_path, migrate_legacy_settings,
        save_theme_library_to_path, THEME_LIBRARY_VERSION,
    };
    use crate::models::{Theme, ThemeColors, ThemeLayers, ThemeLibrary};
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn wallpaper(id: &str, display: &str, accent: &str) -> Theme {
        Theme::wallpaper(
            id.into(),
            "Test wallpaper".into(),
            "test".into(),
            ThemeColors {
                accent: accent.into(),
                secondary: "#445566".into(),
                background: "#112233".into(),
                surface: "#223344".into(),
                foreground: "#F4F7FF".into(),
                muted: "#BBC5D8".into(),
            },
            display.into(),
            "file:///C:/CodeSkin/wallpapers/source.png".into(),
            ThemeLayers::wallpaper(),
        )
    }

    #[test]
    fn selected_wallpaper_round_trips_through_an_independent_storage_path() {
        let path = std::env::temp_dir().join(format!(
            "codeskin-theme-round-trip-{}-themes.json",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let theme = wallpaper(
            "wallpaper-a",
            "file:///C:/CodeSkin/wallpapers/a.jpg",
            "#123456",
        );
        let library = ThemeLibrary {
            version: THEME_LIBRARY_VERSION,
            selected_theme_id: Some(theme.id.clone()),
            themes: vec![theme.clone()],
        };

        save_theme_library_to_path(&path, &library).expect("save selection");
        let recovered = load_theme_library_from_path(&path).expect("new storage reads selection");
        let _ = fs::remove_file(path);

        assert_eq!(recovered.selected_theme_id.as_deref(), Some("wallpaper-a"));
        assert_eq!(recovered.themes[0].background_image, theme.background_image);
        assert_eq!(recovered.themes[0].colors, theme.colors);
    }

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
