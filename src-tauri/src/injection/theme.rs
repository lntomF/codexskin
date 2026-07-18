use crate::models::{Theme, ThemeColors, ThemeLayers};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemePayload {
    pub id: String,
    pub colors: ThemeColors,
    pub background_image: Option<String>,
    pub layers: ThemeLayers,
}

impl ThemePayload {
    pub fn from_theme(theme: &Theme) -> Self {
        Self {
            id: theme.id.clone(),
            colors: theme.colors.clone(),
            background_image: theme.background_image.clone(),
            layers: theme.layers,
        }
    }
}

pub fn install_expression(theme: &Theme) -> Result<String, serde_json::Error> {
    let payload = serde_json::to_string(&ThemePayload::from_theme(theme))?;
    Ok(format!("({})({payload})", super::INSTALL_SCRIPT))
}

#[cfg(test)]
mod tests {
    use super::ThemePayload;
    use crate::models::Theme;

    #[test]
    fn payload_serializes_layers_with_camel_case_names() {
        let theme = Theme::builtin().into_iter().next().expect("built-in theme");
        let payload = serde_json::to_value(ThemePayload::from_theme(&theme)).expect("payload JSON");
        let layers = payload.get("layers").expect("layers object");

        assert!(layers.get("ambientOverlayOpacity").is_some());
        assert!(layers.get("focusOverlayOpacity").is_some());
        assert!(layers.get("sidebarOpacity").is_some());
        assert!(layers.get("cardOpacity").is_some());
    }
}
