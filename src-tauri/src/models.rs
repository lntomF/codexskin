use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const THEME_LIBRARY_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ThemeColors {
    pub accent: String,
    pub background: String,
    pub surface: String,
    pub foreground: String,
    pub muted: String,
}

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
    pub const fn builtin() -> Self {
        Self {
            ambient_overlay_opacity: 0.20,
            focus_overlay_opacity: 0.78,
            sidebar_opacity: 0.58,
            card_opacity: 0.46,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Theme {
    pub id: String,
    pub name: String,
    pub description: String,
    pub colors: ThemeColors,
    pub background_image: Option<String>,
    pub source: ThemeSource,
    pub layers: ThemeLayers,
}

impl Theme {
    pub fn builtin() -> Vec<Self> {
        let layers = ThemeLayers::builtin();
        vec![
            Self {
                id: "midnight-ink".into(),
                name: "Midnight Ink".into(),
                description: "深靛色基底与清晰的蓝紫强调色。".into(),
                colors: ThemeColors {
                    accent: "#8b9dff".into(),
                    background: "#11131f".into(),
                    surface: "#1a1e30".into(),
                    foreground: "#edf0ff".into(),
                    muted: "#aab2ce".into(),
                },
                background_image: None,
                source: ThemeSource::Builtin,
                layers,
            },
            Self {
                id: "forest-terminal".into(),
                name: "Forest Terminal".into(),
                description: "低饱和森林绿，适合长时间阅读。".into(),
                colors: ThemeColors {
                    accent: "#70d6a1".into(),
                    background: "#101814".into(),
                    surface: "#1a2821".into(),
                    foreground: "#e7f5ea".into(),
                    muted: "#a5bcaa".into(),
                },
                background_image: None,
                source: ThemeSource::Builtin,
                layers,
            },
            Self {
                id: "paper-lantern".into(),
                name: "Paper Lantern".into(),
                description: "暖白纸张与琥珀色强调色。".into(),
                colors: ThemeColors {
                    accent: "#b96724".into(),
                    background: "#f6f1e8".into(),
                    surface: "#fffaf0".into(),
                    foreground: "#2b241e".into(),
                    muted: "#74675b".into(),
                },
                background_image: None,
                source: ThemeSource::Builtin,
                layers,
            },
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ThemeLibrary {
    pub version: u32,
    pub selected_theme_id: Option<String>,
    pub themes: Vec<Theme>,
}

impl ThemeLibrary {
    pub fn with_builtin_themes() -> Self {
        Self {
            version: THEME_LIBRARY_VERSION,
            selected_theme_id: None,
            themes: Theme::builtin(),
        }
    }
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
        let style_layer = value
            .get("styleLayer")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let mode = value.get("mode").and_then(Value::as_str).map(str::to_owned);

        let mut missing_markers = Vec::new();
        if !wallpaper_layer {
            missing_markers.push("wallpaperLayer");
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
            active: browser_active && wallpaper_layer && style_layer,
            detail,
            wallpaper_layer,
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
    use super::{TargetVerification, Theme, ThemeSource};
    use serde_json::json;

    #[test]
    fn decodes_complete_browser_verification_value() {
        let browser_value = json!({
            "active": true,
            "themeId": "wallpaper-test",
            "accent": "#112233",
            "wallpaperLayer": true,
            "styleLayer": true,
            "mode": "focus"
        });

        let verification = TargetVerification::from_browser_value(
            "target-1".into(),
            "http://127.0.0.1:9222/".into(),
            &browser_value,
        );

        assert!(verification.active);
        assert!(verification.wallpaper_layer);
        assert!(verification.style_layer);
        assert_eq!(verification.mode.as_deref(), Some("focus"));
        assert!(verification.detail.contains("wallpaper-test"));
        assert!(verification.detail.contains("#112233"));
    }

    #[test]
    fn treats_missing_codeskin_owned_markers_as_inactive() {
        let verification = TargetVerification::from_browser_value(
            "target-2".into(),
            "http://127.0.0.1:9222/".into(),
            &json!({ "active": true, "themeId": "wallpaper-test" }),
        );

        assert!(!verification.active);
        assert!(!verification.wallpaper_layer);
        assert!(!verification.style_layer);
        assert!(verification.mode.is_none());
        assert!(verification.detail.contains("wallpaperLayer"));
        assert!(verification.detail.contains("styleLayer"));
    }

    #[test]
    fn builtin_theme_has_nonempty_css_values() {
        let theme = Theme::builtin().into_iter().next().expect("built-in theme");
        assert!(theme.colors.accent.starts_with('#'));
        assert!(!theme.id.is_empty());
    }

    #[test]
    fn builtin_theme_has_valid_layers() {
        for theme in Theme::builtin() {
            assert_eq!(theme.source, ThemeSource::Builtin);
            assert!(theme.background_image.is_none());
            for opacity in [
                theme.layers.ambient_overlay_opacity,
                theme.layers.focus_overlay_opacity,
                theme.layers.sidebar_opacity,
                theme.layers.card_opacity,
            ] {
                assert!((0.0..=1.0).contains(&opacity));
            }
        }
    }
}
