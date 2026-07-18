use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Version 2 removes bundled colour themes and stores only user-managed backgrounds.
pub const THEME_LIBRARY_VERSION: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ThemeColors {
    /// Primary accent derived from the uploaded wallpaper.
    pub accent: String,
    /// A second image-derived accent for hover, focus, and subtle borders.
    #[serde(default = "default_secondary_color")]
    pub secondary: String,
    pub background: String,
    pub surface: String,
    pub foreground: String,
    pub muted: String,
}

fn default_secondary_color() -> String {
    "#8B9DFF".into()
}

/// Kept solely to read CodeSkin v1 data. New entries are always `Wallpaper`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ThemeSource {
    Builtin,
    Wallpaper,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ThemeLayers {
    pub ambient_overlay_opacity: f32,
    pub focus_overlay_opacity: f32,
    pub sidebar_opacity: f32,
    pub card_opacity: f32,
}

impl ThemeLayers {
    pub const fn wallpaper() -> Self {
        // These are deliberately low: the wallpaper is a clear background layer,
        // while only specific controls receive a glass treatment in the renderer.
        Self {
            ambient_overlay_opacity: 0.12,
            focus_overlay_opacity: 0.18,
            sidebar_opacity: 0.12,
            card_opacity: 0.18,
        }
    }
}

/// Readability and local glass settings measured from one area of the derived
/// 16:9 wallpaper. Values are recomputed whenever the wallpaper is applied.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ContrastRegion {
    pub luminance: f32,
    pub complexity: f32,
    pub foreground: String,
    pub muted: String,
    pub panel_color: String,
    pub panel_opacity: f32,
    pub blur_px: u8,
    pub text_shadow: String,
}

/// Region-specific contrast prevents a dark or visually busy part of a
/// wallpaper from inheriting a text colour chosen for a different area.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ThemeContrast {
    pub sidebar: ContrastRegion,
    pub content: ContrastRegion,
    /// Top title/application-menu strip sampled across the actual wallpaper header area.
    /// Optional keeps libraries written before this region was introduced readable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header: Option<ContrastRegion>,
    pub info_panel: ContrastRegion,
    pub composer: ContrastRegion,
}

/// Internal injection payload. It represents one user-uploaded Codex background;
/// the historical name remains to avoid a risky, unrelated CDP-layer rewrite.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Theme {
    pub id: String,
    pub name: String,
    pub description: String,
    pub colors: ThemeColors,
    /// The generated 2560 x 1440 JPEG used by Codex and by the thumbnail grid.
    pub background_image: Option<String>,
    /// The original uploaded image retained under CodeSkin's managed wallpaper folder.
    #[serde(default)]
    pub source_image: Option<String>,
    pub source: ThemeSource,
    pub layers: ThemeLayers,
    /// `None` is retained only for libraries created before area-aware contrast.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contrast: Option<ThemeContrast>,
}

impl Theme {
    pub fn wallpaper(
        id: String,
        name: String,
        description: String,
        colors: ThemeColors,
        background_image: String,
        source_image: String,
        layers: ThemeLayers,
    ) -> Self {
        Self {
            id,
            name,
            description,
            colors,
            background_image: Some(background_image),
            source_image: Some(source_image),
            source: ThemeSource::Wallpaper,
            layers,
            contrast: None,
        }
    }
}

/// Serialized as a background library. `themes` and `selectedThemeId` aliases make
/// v1 `themes.json` files readable once, after which they are saved in v2 form.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThemeLibrary {
    pub version: u32,
    #[serde(rename = "selectedBackgroundId", alias = "selectedThemeId")]
    pub selected_theme_id: Option<String>,
    #[serde(rename = "backgrounds", alias = "themes")]
    pub themes: Vec<Theme>,
}

impl ThemeLibrary {
    pub fn empty() -> Self {
        Self {
            version: THEME_LIBRARY_VERSION,
            selected_theme_id: None,
            themes: Vec::new(),
        }
    }
}

/// IPC-only metadata for the UI. `preview_data_url` is generated on demand and
/// is never serialized into `%LOCALAPPDATA%\CodeSkin\themes.json`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackgroundView {
    pub id: String,
    pub name: String,
    pub description: String,
    pub background_image: Option<String>,
    pub source_image: Option<String>,
    pub preview_data_url: Option<String>,
}

impl BackgroundView {
    pub fn from_theme(theme: Theme, preview_data_url: Option<String>) -> Self {
        Self {
            id: theme.id,
            name: theme.name,
            description: theme.description,
            background_image: theme.background_image,
            source_image: theme.source_image,
            preview_data_url,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackgroundLibraryView {
    pub version: u32,
    pub selected_background_id: Option<String>,
    pub backgrounds: Vec<BackgroundView>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CodexConnectionState {
    NotRunning,
    RunningWithoutDebugPort,
    DebugPortDetected,
    Starting,
    Connecting,
    Connected,
    Reconnecting,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexStatus {
    pub state: CodexConnectionState,
    pub port: Option<u16>,
    pub executable_path: Option<String>,
    pub detail: String,
}

impl CodexStatus {
    pub fn not_running() -> Self {
        Self {
            state: CodexConnectionState::NotRunning,
            port: None,
            executable_path: None,
            detail: "未检测到 Codex.exe。".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetVerification {
    pub target_id: String,
    pub target_url: String,
    pub active: bool,
    pub detail: String,
    pub wallpaper_layer: bool,
    pub wallpaper_configured: bool,
    pub style_layer: bool,
    pub mode: Option<String>,
}

impl TargetVerification {
    pub fn from_browser_value(target_id: String, target_url: String, value: &Value) -> Self {
        let browser_active = value
            .get("active")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let wallpaper_layer = value
            .get("wallpaperLayer")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let wallpaper_configured = value
            .get("wallpaperConfigured")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let style_layer = value
            .get("styleLayer")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let mode = value.get("mode").and_then(Value::as_str).map(str::to_owned);

        let mut missing_markers = Vec::new();
        if !wallpaper_layer {
            missing_markers.push("wallpaperLayer");
        }
        if !wallpaper_configured {
            missing_markers.push("wallpaperConfigured");
        }
        if !style_layer {
            missing_markers.push("styleLayer");
        }

        let detail = if !missing_markers.is_empty() {
            format!(
                "缺少 CodeSkin-owned marker：{}。浏览器验证：{}",
                missing_markers.join("、"),
                value
            )
        } else if !browser_active {
            format!("CodeSkin 注入未激活。浏览器验证：{value}")
        } else {
            value.to_string()
        };

        Self {
            target_id,
            target_url,
            active: browser_active && wallpaper_layer && wallpaper_configured && style_layer,
            detail,
            wallpaper_layer,
            wallpaper_configured,
            style_layer,
            mode,
        }
    }

    pub fn failed(target_id: String, target_url: String, detail: String) -> Self {
        Self {
            target_id,
            target_url,
            active: false,
            detail,
            wallpaper_layer: false,
            wallpaper_configured: false,
            style_layer: false,
            mode: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyResult {
    pub theme_id: Option<String>,
    pub active: bool,
    pub targets: Vec<TargetVerification>,
}

#[cfg(test)]
mod tests {
    use super::{TargetVerification, Theme, ThemeColors, ThemeLayers, ThemeSource};
    use serde_json::json;

    #[test]
    fn decodes_complete_browser_verification_value() {
        let browser_value = json!({
            "active": true,
            "themeId": "wallpaper-test",
            "accent": "#112233",
            "wallpaperLayer": true,
            "wallpaperConfigured": true,
            "styleLayer": true,
            "mode": "focus"
        });
        let verification = TargetVerification::from_browser_value(
            "target-1".into(),
            "http://127.0.0.1:9222/".into(),
            &browser_value,
        );
        assert!(verification.active);
        assert_eq!(verification.mode.as_deref(), Some("focus"));
    }

    #[test]
    fn legacy_contrast_without_header_deserializes_and_omits_missing_header_on_reserialize() {
        let region = json!({
            "luminance": 0.25,
            "complexity": 0.1,
            "foreground": "#F4F7FF",
            "muted": "#BBC5D8",
            "panelColor": "#12161D",
            "panelOpacity": 0.2,
            "blurPx": 8,
            "textShadow": "0 1px 2px rgba(0,0,0,0.42)"
        });
        let contrast = serde_json::from_value::<super::ThemeContrast>(json!({
            "sidebar": region.clone(),
            "content": region.clone(),
            "infoPanel": region.clone(),
            "composer": region
        }))
        .expect("legacy contrast should deserialize");

        assert!(contrast.header.is_none());
        let reserialized = serde_json::to_value(contrast).expect("contrast JSON");
        assert!(reserialized.get("header").is_none());
    }

    #[test]
    fn wallpaper_keeps_generated_and_original_urls() {
        let background = Theme::wallpaper(
            "wallpaper-test".into(),
            "Test".into(),
            "test".into(),
            ThemeColors {
                accent: "#112233".into(),
                secondary: "#8B9DFF".into(),
                background: "#111111".into(),
                surface: "#222222".into(),
                foreground: "#FFFFFF".into(),
                muted: "#AAAAAA".into(),
            },
            "file:///C:/CodeSkin/wallpapers/display.jpg".into(),
            "file:///C:/CodeSkin/wallpapers/source.png".into(),
            ThemeLayers::wallpaper(),
        );
        assert_eq!(background.source, ThemeSource::Wallpaper);
        assert!(background.background_image.is_some());
        assert!(background.source_image.is_some());
    }
}
