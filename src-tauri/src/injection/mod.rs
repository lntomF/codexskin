mod registry;
mod scripts;
mod theme;
pub use registry::{InjectionRegistry, RegisteredTarget};
pub use scripts::{INSTALL_SCRIPT, RESTORE_SCRIPT, VERIFY_SCRIPT};
pub use theme::install_expression;
#[cfg(test)]
mod tests {
    use super::{theme::ThemePayload, INSTALL_SCRIPT, RESTORE_SCRIPT, VERIFY_SCRIPT};
    use std::process::Command;
    #[test]
    fn install_script_uses_only_codeskin_owned_markers() {
        assert!(INSTALL_SCRIPT.contains("codeskin-runtime-style"));
        assert!(RESTORE_SCRIPT.contains("codeskin-runtime-style"));
        assert!(!INSTALL_SCRIPT.contains("app.asar"));
    }
    #[test]
    fn install_script_owns_a_noninteractive_wallpaper_layer() {
        assert!(INSTALL_SCRIPT.contains("codeskin-wallpaper-layer"));
        assert!(INSTALL_SCRIPT.contains("pointer-events: none"));
        assert!(INSTALL_SCRIPT.contains("data-codeskin-mode"));
        assert!(!INSTALL_SCRIPT.contains("document.body.innerHTML"));
    }
    #[test]
    fn wallpaper_is_layered_below_codex_with_regional_contrast_safe_glass() {
        assert!(INSTALL_SCRIPT.contains("z-index: 0;"));
        assert!(INSTALL_SCRIPT.contains("] #root {"));
        assert!(INSTALL_SCRIPT.contains("z-index: 1;"));
        assert!(INSTALL_SCRIPT.contains("::after"));
        assert!(INSTALL_SCRIPT.contains("--codeskin-wallpaper-veil"));
        assert!(INSTALL_SCRIPT.contains("opacity: var(--codeskin-current-overlay-opacity);"));
        assert!(INSTALL_SCRIPT.contains(".main-surface"));
        assert!(INSTALL_SCRIPT.contains("background: transparent !important;"));
        assert!(INSTALL_SCRIPT.contains(".app-shell-left-panel"));
        assert!(INSTALL_SCRIPT.contains(".composer-surface-chrome"));
        assert!(INSTALL_SCRIPT.contains("--codeskin-sidebar-panel-color"));
        assert!(INSTALL_SCRIPT.contains("--codeskin-sidebar-panel-opacity"));
        assert!(INSTALL_SCRIPT.contains("--codeskin-content-panel-color"));
        assert!(INSTALL_SCRIPT.contains("--codeskin-content-panel-opacity"));
        assert!(INSTALL_SCRIPT.contains("--codeskin-info-panel-color"));
        assert!(INSTALL_SCRIPT.contains("--codeskin-composer-panel-color"));
        assert!(INSTALL_SCRIPT.contains("--codeskin-composer-panel-opacity"));
        assert!(INSTALL_SCRIPT.contains("backdrop-filter: blur(var(--codeskin-sidebar-blur))"));
        assert!(INSTALL_SCRIPT.contains("backdrop-filter: blur(var(--codeskin-composer-blur))"));
        assert!(!INSTALL_SCRIPT.contains("--codeskin-glass-color"));
        assert!(!INSTALL_SCRIPT.contains("#11151C"));
        assert!(INSTALL_SCRIPT.contains("group/home-suggestions"));
        assert!(!INSTALL_SCRIPT.contains(":root[data-codeskin-theme-id] button,"));
    }
    #[test]
    fn menus_dialogs_and_dropdowns_use_their_sampled_regional_glass() {
        assert!(INSTALL_SCRIPT.contains("button.no-drag[aria-haspopup=\"menu\"]"));
        assert!(INSTALL_SCRIPT.contains("[role=\"menu\"]"));
        assert!(INSTALL_SCRIPT.contains("[data-radix-menu-content]"));
        assert!(INSTALL_SCRIPT.contains("[role=\"dialog\"]"));
        assert!(INSTALL_SCRIPT.contains("[role=\"listbox\"]"));
        assert!(INSTALL_SCRIPT.contains("bg-token-dropdown-background"));
        assert!(INSTALL_SCRIPT.contains("var(--codeskin-sidebar-elevated-opacity)"));
        assert!(INSTALL_SCRIPT.contains("var(--codeskin-sidebar-blur)"));
        assert!(INSTALL_SCRIPT.contains("var(--codeskin-info-elevated-opacity)"));
        assert!(INSTALL_SCRIPT.contains("var(--codeskin-info-blur)"));
        assert!(INSTALL_SCRIPT.contains("[data-highlighted]"));
        assert!(INSTALL_SCRIPT.contains("::placeholder"));
        assert!(INSTALL_SCRIPT.contains(":disabled"));
        assert!(INSTALL_SCRIPT.contains("aria-expanded"));
        assert!(INSTALL_SCRIPT.contains("data-state"));
    }
    #[test]
    fn payload_json_escapes_theme_values() {
        let payload = ThemePayload {
            id: "wallpaper-test".into(),
            colors: crate::models::ThemeColors {
                accent: "#112233".into(),
                secondary: "#8B9DFF".into(),
                background: "#111111".into(),
                surface: "#222222".into(),
                foreground: "#FFFFFF".into(),
                muted: "#AAAAAA".into(),
            },
            background_image: Some("data:image/jpeg;base64,/9j/2Q==".into()),
            layers: crate::models::ThemeLayers::wallpaper(),
            contrast: None,
        };
        assert!(serde_json::to_string(&payload).is_ok());
    }
    #[test]
    fn install_script_uses_camel_case_layer_parameters() {
        assert!(INSTALL_SCRIPT.contains("ambientOverlayOpacity"));
        assert!(INSTALL_SCRIPT.contains("focusOverlayOpacity"));
    }
    #[test]
    fn install_script_defers_when_document_root_is_not_ready() {
        assert!(INSTALL_SCRIPT.contains("DOMContentLoaded"));
        assert!(INSTALL_SCRIPT.contains("document.documentElement"));
    }
    #[test]
    fn deferred_install_is_owned_and_restore_can_cancel_its_exact_handler() {
        assert!(INSTALL_SCRIPT.contains("pendingInstall"));
        assert!(INSTALL_SCRIPT
            .contains("removeEventListener(\"DOMContentLoaded\", runtime.pendingInstall)"));
        assert!(RESTORE_SCRIPT
            .contains("removeEventListener(\"DOMContentLoaded\", runtime.pendingInstall)"));
        assert!(RESTORE_SCRIPT.contains("runtime.pendingInstall = null"));
    }
    #[test]
    fn runtime_record_is_the_only_cleanup_and_verification_authority() {
        for script in [INSTALL_SCRIPT, RESTORE_SCRIPT, VERIFY_SCRIPT] {
            assert!(script.contains("__codeskinRuntime"));
            assert!(script.contains("codeskin-runtime-v1"));
        }
        assert!(!RESTORE_SCRIPT.contains("querySelectorAll"));
        assert!(!VERIFY_SCRIPT.contains("querySelector("));
        assert!(INSTALL_SCRIPT.contains("__codeskinModeObserver"));
        assert!(RESTORE_SCRIPT.contains("__codeskinModeObserver"));
        assert!(VERIFY_SCRIPT.contains("__codeskinModeObserver"));
    }
    #[test]
    fn install_script_accepts_only_a_bounded_jpeg_data_url() {
        assert!(INSTALL_SCRIPT.contains("/^#[0-9A-Fa-f]{6}$/"));
        assert!(INSTALL_SCRIPT.contains("FALLBACK_COLORS"));
        assert!(INSTALL_SCRIPT.contains("safeBackgroundImage"));
        assert!(INSTALL_SCRIPT.contains("JPEG_DATA_URL"));
        assert!(INSTALL_SCRIPT.contains("MAX_JPEG_DATA_URL_LENGTH"));
        assert!(INSTALL_SCRIPT.contains("data:image\\/jpeg;base64"));
        assert!(!INSTALL_SCRIPT.contains("parsed.protocol"));
        assert!(!INSTALL_SCRIPT.contains("WINDOWS_ABSOLUTE_PATH"));
    }
    #[test]
    fn mode_observer_coalesces_updates_and_uses_working_content_as_focus_signal() {
        assert!(INSTALL_SCRIPT.contains("queueMicrotask"));
        assert!(INSTALL_SCRIPT.contains("root.getAttribute(modeAttribute) !== mode"));
        assert!(INSTALL_SCRIPT.contains("hasMain && !hasTranscript && !hasCode"));
        assert!(INSTALL_SCRIPT.contains("? \"ambient\" : \"focus\""));
        assert!(!INSTALL_SCRIPT.contains("hasWelcomeSurface"));
    }
    #[test]
    fn observer_global_is_owned_by_the_runtime_record() {
        assert!(INSTALL_SCRIPT.contains("observer-conflict"));
        assert!(INSTALL_SCRIPT.contains("observerGlobalIsCompatible"));
        assert!(INSTALL_SCRIPT.contains("window[observerKey] = observer"));
        assert!(RESTORE_SCRIPT.contains("window[observerKey] === runtime.observer"));
        assert!(VERIFY_SCRIPT.contains("observerGlobalMatchesRuntime"));
        assert!(VERIFY_SCRIPT.contains("safe,"));
    }
    #[test]
    fn restore_script_removes_wallpaper_layer_and_mode_observer_state() {
        assert!(RESTORE_SCRIPT.contains("codeskin-wallpaper-layer"));
        assert!(RESTORE_SCRIPT.contains("data-codeskin-mode"));
        assert!(RESTORE_SCRIPT.contains("disconnect"));
        assert!(RESTORE_SCRIPT.contains("delete window[observerKey]"));
    }
    #[test]
    fn pending_runtime_cleanup_matches_browser_behavior() {
        let test_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/injection/runtime_behavior_test.js"
        );
        let output = Command::new("node")
            .arg(test_path)
            .output()
            .expect("Node.js must be available to run injection runtime behavior tests");
        assert!(
            output.status.success(),
            "Node behavior test failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn verify_script_reports_wallpaper_style_and_mode_contract() {
        assert!(VERIFY_SCRIPT.contains("codeskin-wallpaper-layer"));
        assert!(VERIFY_SCRIPT.contains("active:"));
        assert!(VERIFY_SCRIPT.contains("themeId"));
        assert!(VERIFY_SCRIPT.contains("accent"));
        assert!(VERIFY_SCRIPT.contains("wallpaperLayer"));
        assert!(VERIFY_SCRIPT.contains("styleLayer"));
        assert!(VERIFY_SCRIPT.contains("mode"));
        assert!(VERIFY_SCRIPT.contains("data-codeskin-mode"));
        assert!(VERIFY_SCRIPT.contains("validThemeId"));
        assert!(VERIFY_SCRIPT.contains("validMode"));
    }
}
