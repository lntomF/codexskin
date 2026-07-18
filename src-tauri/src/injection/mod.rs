mod registry;
mod scripts;
mod theme;
pub use registry::{InjectionRegistry, RegisteredTarget};
pub use scripts::{INSTALL_SCRIPT, RESTORE_SCRIPT, VERIFY_SCRIPT};
pub use theme::install_expression;
#[cfg(test)]
mod tests {
    use super::{
        install_expression, theme::ThemePayload, INSTALL_SCRIPT, RESTORE_SCRIPT, VERIFY_SCRIPT,
    };
    use crate::models::Theme;
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
    fn payload_json_escapes_theme_values() {
        let theme = Theme::builtin().into_iter().next().expect("built-in theme");
        let payload = ThemePayload::from_theme(&theme);
        assert!(serde_json::to_string(&payload).is_ok());
    }
    #[test]
    fn install_expression_contains_camel_case_layer_parameters() {
        let theme = Theme::builtin().into_iter().next().expect("built-in theme");
        let expression = install_expression(&theme).expect("install expression");
        assert!(expression.contains("ambientOverlayOpacity"));
        assert!(expression.contains("focusOverlayOpacity"));
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
    fn install_script_rejects_unsafe_css_colors_and_remote_wallpapers() {
        assert!(INSTALL_SCRIPT.contains("/^#[0-9A-Fa-f]{6}$/"));
        assert!(INSTALL_SCRIPT.contains("FALLBACK_COLORS"));
        assert!(INSTALL_SCRIPT.contains("safeBackgroundImage"));
        assert!(INSTALL_SCRIPT.contains("parsed.protocol !== \"file:\""));
        assert!(INSTALL_SCRIPT.contains("parsed.hostname !== \"\""));
        assert!(INSTALL_SCRIPT.contains("parsed.pathname.startsWith(\"//\")"));
        assert!(INSTALL_SCRIPT.contains("WINDOWS_ABSOLUTE_PATH"));
    }
    #[test]
    fn mode_observer_coalesces_updates_and_defaults_incomplete_dom_to_focus() {
        assert!(INSTALL_SCRIPT.contains("queueMicrotask"));
        assert!(INSTALL_SCRIPT.contains("root.getAttribute(modeAttribute) !== mode"));
        assert!(INSTALL_SCRIPT.contains("hasMain && hasWelcomeSurface && !hasWorkingSurface"));
        assert!(INSTALL_SCRIPT.contains(": \"focus\""));
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
