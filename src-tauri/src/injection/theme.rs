use crate::{
    error::CommandError,
    models::{Theme, ThemeColors, ThemeContrast, ThemeLayers},
};
use image::ImageFormat;
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

const MAX_INJECTION_JPEG_BYTES: usize = 16 * 1024 * 1024;
const BASE64_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemePayload {
    pub id: String,
    pub colors: ThemeColors,
    pub background_image: Option<String>,
    pub layers: ThemeLayers,
    pub contrast: Option<ThemeContrast>,
}

impl ThemePayload {
    /// Builds the payload sent to Codex. The persisted theme retains a local
    /// file URL for UI previews, but Codex's `app://` renderer does not load
    /// `file:///` subresources reliably. Only CodeSkin-managed JPEGs are read
    /// and embedded as a data URL for the loopback-only CDP injection.
    pub fn for_injection(theme: &Theme) -> Result<Self, CommandError> {
        Self::for_injection_with_background(
            theme,
            theme
                .background_image
                .as_deref()
                .map(managed_jpeg_data_url)
                .transpose()?,
        )
    }

    fn for_injection_with_background(
        theme: &Theme,
        background_image: Option<String>,
    ) -> Result<Self, CommandError> {
        Ok(Self {
            id: theme.id.clone(),
            colors: theme.colors.clone(),
            background_image,
            layers: theme.layers,
            contrast: theme.contrast.clone(),
        })
    }

    #[cfg(test)]
    pub(crate) fn for_injection_from_root(
        theme: &Theme,
        root: &Path,
    ) -> Result<Self, CommandError> {
        Self::for_injection_with_background(
            theme,
            theme
                .background_image
                .as_deref()
                .map(|url| managed_jpeg_data_url_from_root(url, root))
                .transpose()?,
        )
    }
}

pub fn install_expression(theme: &Theme) -> Result<String, CommandError> {
    let payload = ThemePayload::for_injection(theme)?;
    let payload = serde_json::to_string(&payload)
        .map_err(|error| CommandError::new("theme_payload_serialize_failed", error.to_string()))?;
    Ok(format!("({})({payload})", super::INSTALL_SCRIPT))
}

fn managed_jpeg_data_url(file_url: &str) -> Result<String, CommandError> {
    managed_jpeg_data_url_from_root(file_url, &managed_wallpaper_root()?)
}

fn managed_jpeg_data_url_from_root(file_url: &str, root: &Path) -> Result<String, CommandError> {
    let path = file_url_to_local_path(file_url)?;
    let canonical_root = root.canonicalize().map_err(|error| {
        CommandError::new(
            "background_storage_unavailable",
            format!("无法访问 CodeSkin 壁纸目录：{error}"),
        )
    })?;
    let canonical_path = path.canonicalize().map_err(|error| {
        CommandError::new(
            "background_file_unavailable",
            format!("无法访问已选背景图：{error}"),
        )
    })?;

    if !canonical_path.starts_with(&canonical_root) || !canonical_path.is_file() {
        return Err(CommandError::new(
            "background_not_managed",
            "只允许注入 CodeSkin 管理的本地背景图。",
        ));
    }

    let bytes = fs::read(&canonical_path).map_err(|error| {
        CommandError::new(
            "background_read_failed",
            format!("读取已选背景图失败：{error}"),
        )
    })?;
    if bytes.is_empty() || bytes.len() > MAX_INJECTION_JPEG_BYTES {
        return Err(CommandError::new(
            "background_injection_size_invalid",
            "用于注入的背景图为空或超过 16 MiB。",
        ));
    }
    let format = image::guess_format(&bytes).map_err(|error| {
        CommandError::new(
            "background_injection_decode_failed",
            format!("无法识别已选背景图：{error}"),
        )
    })?;
    if format != ImageFormat::Jpeg {
        return Err(CommandError::new(
            "background_injection_format_invalid",
            "用于注入的背景图必须是 CodeSkin 派生的 JPEG。",
        ));
    }

    Ok(format!("data:image/jpeg;base64,{}", base64_encode(&bytes)))
}

fn managed_wallpaper_root() -> Result<PathBuf, CommandError> {
    let local_app_data = std::env::var_os("LOCALAPPDATA")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            CommandError::new(
                "background_local_app_data_unavailable",
                "无法读取 LOCALAPPDATA，不能确定壁纸存储目录。",
            )
        })?;
    Ok(PathBuf::from(local_app_data)
        .join("CodeSkin")
        .join("wallpapers"))
}

fn file_url_to_local_path(file_url: &str) -> Result<PathBuf, CommandError> {
    let path = file_url.strip_prefix("file:///").ok_or_else(|| {
        CommandError::new(
            "background_url_invalid",
            "背景图不是受支持的本地 file:/// 路径。",
        )
    })?;
    if path.is_empty()
        || path.starts_with('/')
        || path.contains('?')
        || path.contains('#')
        || !matches!(path.as_bytes(), [drive, b':', slash, ..] if drive.is_ascii_alphabetic() && (*slash == b'/' || *slash == b'\\'))
    {
        return Err(CommandError::new(
            "background_url_invalid",
            "背景图不是受支持的 Windows 本地 file:/// 路径。",
        ));
    }
    Ok(PathBuf::from(path.replace('/', "\\")))
}

fn base64_encode(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = *chunk.get(1).unwrap_or(&0);
        let third = *chunk.get(2).unwrap_or(&0);
        encoded.push(BASE64_ALPHABET[(first >> 2) as usize] as char);
        encoded
            .push(BASE64_ALPHABET[(((first & 0b0000_0011) << 4) | (second >> 4)) as usize] as char);
        encoded.push(if chunk.len() > 1 {
            BASE64_ALPHABET[(((second & 0b0000_1111) << 2) | (third >> 6)) as usize] as char
        } else {
            '='
        });
        encoded.push(if chunk.len() > 2 {
            BASE64_ALPHABET[(third & 0b0011_1111) as usize] as char
        } else {
            '='
        });
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::{base64_encode, ThemePayload};
    use crate::models::{Theme, ThemeColors, ThemeLayers, ThemeSource};
    use image::{codecs::jpeg::JpegEncoder, Rgb, RgbImage};
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temporary_wallpaper_root() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let root = std::env::temp_dir()
            .join(format!("codeskin-theme-test-{unique}"))
            .join("wallpapers");
        fs::create_dir_all(&root).expect("create test wallpaper root");
        root
    }

    fn fixture_theme(file_url: String) -> Theme {
        Theme {
            id: "wallpaper-test".into(),
            name: "Wallpaper".into(),
            description: "fixture".into(),
            colors: ThemeColors {
                accent: "#112233".into(),
                secondary: "#8B9DFF".into(),
                background: "#111111".into(),
                surface: "#222222".into(),
                foreground: "#FFFFFF".into(),
                muted: "#AAAAAA".into(),
            },
            background_image: Some(file_url),
            source_image: None,
            source: ThemeSource::Wallpaper,
            layers: ThemeLayers::wallpaper(),
            contrast: None,
        }
    }

    fn file_url(path: &std::path::Path) -> String {
        format!("file:///{}", path.to_string_lossy().replace('\\', "/"))
    }

    #[test]
    fn base64_encoder_handles_all_padding_variants() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
    }

    #[test]
    fn managed_display_jpeg_is_embedded_as_a_data_url() {
        let root = temporary_wallpaper_root();
        let display = root.join("display.jpg");
        let image = RgbImage::from_pixel(2, 2, Rgb([32, 64, 128]));
        let file = fs::File::create(&display).expect("create JPEG");
        JpegEncoder::new_with_quality(file, 90)
            .encode_image(&image)
            .expect("write JPEG");

        let theme = fixture_theme(file_url(&display));
        let payload =
            ThemePayload::for_injection_from_root(&theme, &root).expect("injection payload");
        assert!(payload
            .background_image
            .as_deref()
            .is_some_and(|value| value.starts_with("data:image/jpeg;base64,/9j/")));
        assert!(!payload
            .background_image
            .as_deref()
            .unwrap()
            .contains("file:///"));

        fs::remove_dir_all(root.parent().expect("test root parent")).expect("remove test root");
    }

    #[test]
    fn rejects_wallpaper_outside_codeskin_managed_root() {
        let root = temporary_wallpaper_root();
        let outside = root.parent().expect("root parent").join("outside.jpg");
        fs::write(&outside, [0xFF, 0xD8, 0xFF, 0xD9]).expect("write outside JPEG");
        let theme = fixture_theme(file_url(&outside));

        let error = ThemePayload::for_injection_from_root(&theme, &root)
            .expect_err("must reject outside path");
        assert_eq!(error.code, "background_not_managed");

        fs::remove_dir_all(root.parent().expect("test root parent")).expect("remove test root");
    }

    #[test]
    fn payload_serializes_layers_with_camel_case_names() {
        let payload = ThemePayload {
            id: "wallpaper-test".into(),
            colors: ThemeColors {
                accent: "#112233".into(),
                secondary: "#8B9DFF".into(),
                background: "#111111".into(),
                surface: "#222222".into(),
                foreground: "#FFFFFF".into(),
                muted: "#AAAAAA".into(),
            },
            background_image: Some("data:image/jpeg;base64,/9j/2Q==".into()),
            layers: ThemeLayers::wallpaper(),
            contrast: None,
        };
        let payload = serde_json::to_value(payload).expect("payload JSON");
        let layers = payload.get("layers").expect("layers object");

        assert!(layers.get("ambientOverlayOpacity").is_some());
        assert!(layers.get("focusOverlayOpacity").is_some());
        assert!(layers.get("sidebarOpacity").is_some());
        assert!(layers.get("cardOpacity").is_some());
    }
}
