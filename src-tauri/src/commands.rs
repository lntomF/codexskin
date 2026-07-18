use crate::{
    app_state::AppState,
    error::CommandError,
    models::{BackgroundLibraryView, BackgroundView, CodexStatus, ThemeLibrary, VerifyResult},
    process, storage,
};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub fn inspect_codex_status() -> CodexStatus {
    process::inspect_running_codex()
}

#[tauri::command]
pub fn load_background_library() -> Result<BackgroundLibraryView, CommandError> {
    Ok(background_library_view(storage::load_theme_library()?))
}

#[tauri::command]
pub async fn connect_or_start_codex(
    state: State<'_, Arc<AppState>>,
) -> Result<CodexStatus, CommandError> {
    state.connect_or_start_codex().await
}

#[tauri::command]
pub async fn apply_background(
    background_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<VerifyResult, CommandError> {
    apply_saved_background_by_id(&background_id, state.inner()).await
}

pub(crate) async fn apply_saved_background_by_id(
    background_id: &str,
    state: &Arc<AppState>,
) -> Result<VerifyResult, CommandError> {
    let mut library = storage::load_theme_library()?;
    let index = library
        .themes
        .iter()
        .position(|background| background.id == background_id)
        .ok_or_else(|| CommandError::new("background_not_found", "找不到请求的背景图。"))?;
    let mut background = library.themes[index].clone();
    let display_image = background.background_image.as_deref().ok_or_else(|| {
        CommandError::new("background_display_missing", "该背景缺少可应用的派生壁纸。")
    })?;
    // Re-analyse the same centre-cropped 16:9 JPEG rendered by Codex. This makes
    // text contrast, panel opacity, and blur react every time a background is applied.
    let display_bytes = storage::read_managed_background_bytes(display_image)?;
    storage::refresh_wallpaper_theme_visuals(&mut background, &display_bytes)?;
    library.themes[index] = background.clone();

    let result = state.apply_saved_theme(background).await?;
    library.selected_theme_id = Some(background_id.to_owned());
    storage::save_theme_library(&library)?;
    state.start_reconnector();
    Ok(result)
}

#[tauri::command]
pub fn import_background(
    bytes: Vec<u8>,
    display_name: String,
) -> Result<BackgroundLibraryView, CommandError> {
    let name = display_name.trim();
    let mut background = storage::import_wallpaper_theme(
        &bytes,
        if name.is_empty() {
            "未命名背景"
        } else {
            name
        },
    )?;
    let mut library = storage::load_theme_library()?;
    background.id = unique_background_id(&background.id, &library);
    library.themes.push(background);
    storage::save_theme_library(&library)?;
    Ok(background_library_view(library))
}

#[tauri::command]
pub async fn delete_background(
    background_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<BackgroundLibraryView, CommandError> {
    let mut library = storage::load_theme_library()?;
    let index = library
        .themes
        .iter()
        .position(|background| background.id == background_id)
        .ok_or_else(|| CommandError::new("background_not_found", "找不到要删除的背景图。"))?;

    // Do not remove the active wallpaper until its CDP layers and new-document script are gone.
    if library.selected_theme_id.as_deref() == Some(background_id.as_str()) {
        state.restore_theme().await?;
        library.selected_theme_id = None;
    }

    let background = library.themes.remove(index);
    storage::delete_managed_background_files([
        background.background_image,
        background.source_image,
    ])?;
    storage::save_theme_library(&library)?;
    Ok(background_library_view(library))
}

fn background_library_view(library: ThemeLibrary) -> BackgroundLibraryView {
    let backgrounds = library
        .themes
        .into_iter()
        .map(|background| {
            let preview_data_url =
                storage::wallpaper_preview_data_url(background.background_image.as_deref());
            BackgroundView::from_theme(background, preview_data_url)
        })
        .collect();
    BackgroundLibraryView {
        version: library.version,
        selected_background_id: library.selected_theme_id,
        backgrounds,
    }
}

#[tauri::command]
pub async fn verify_injection(
    state: State<'_, Arc<AppState>>,
) -> Result<VerifyResult, CommandError> {
    state.verify_theme().await
}

pub(crate) async fn restore_original_appearance_inner(
    state: &Arc<AppState>,
) -> Result<VerifyResult, CommandError> {
    let result = state.restore_theme().await?;
    let mut library = storage::load_theme_library()?;
    library.selected_theme_id = None;
    storage::save_theme_library(&library)?;
    Ok(result)
}

#[tauri::command]
pub async fn restore_original_appearance(
    state: State<'_, Arc<AppState>>,
) -> Result<VerifyResult, CommandError> {
    restore_original_appearance_inner(state.inner()).await
}

fn unique_background_id(base_id: &str, library: &ThemeLibrary) -> String {
    if !library
        .themes
        .iter()
        .any(|background| background.id == base_id)
    {
        return base_id.to_owned();
    }
    let mut suffix = 2_u32;
    loop {
        let candidate = format!("{base_id}-{suffix}");
        if !library
            .themes
            .iter()
            .any(|background| background.id == candidate)
        {
            return candidate;
        }
        suffix = suffix.saturating_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::{apply_saved_background_by_id, unique_background_id};
    use crate::{
        app_state::AppState,
        cdp::{discover_page_targets, CdpClient},
        models::{ThemeLibrary, ThemeSource},
        process, storage,
    };
    use serde_json::{json, Value};
    use std::{fs, time::Duration};

    #[tokio::test]
    #[ignore = "requires a running Codex Desktop instance with loopback CDP and three saved CodeSkin wallpapers"]
    async fn prints_live_region_foreground_styles_for_saved_wallpapers() {
        let state = AppState::new();
        let verification = async {
            let library = storage::load_theme_library()?;
            let background_ids = library
                .themes
                .iter()
                .filter(|theme| {
                    theme.source == ThemeSource::Wallpaper && theme.background_image.is_some()
                })
                .map(|theme| theme.id.clone())
                .collect::<Vec<_>>();
            assert!(
                background_ids.len() >= 3,
                "live verification requires at least three saved wallpapers"
            );

            let mut reports = Vec::new();
            for (index, background_id) in background_ids.into_iter().take(3).enumerate() {
                let applied = apply_saved_background_by_id(&background_id, &state).await?;
                assert!(applied.active, "theme {background_id} did not verify as active");
                tokio::time::sleep(Duration::from_millis(250)).await;

                let status = process::inspect_running_codex();
                let port = status
                    .port
                    .expect("live verification requires a detected loopback CDP port");
                let target = discover_page_targets(port)
                    .await?
                    .into_iter()
                    .find(|target| target.url.contains("index.html"))
                    .expect("live verification requires an index.html page target");
                let client = CdpClient::connect(&target.websocket_url, port).await?;
                let report = evaluate_live_region_styles(&client).await?;
                assert_eq!(
                    report["themeId"].as_str(),
                    Some(background_id.as_str()),
                    "the live root must expose the applied theme id"
                );
                assert_eq!(report["info"]["color"], report["info"]["expected"]);
                assert_eq!(report["topFile"]["color"], report["topFile"]["expected"]);
                let view_menu = report["viewMenu"]
                    .as_array()
                    .expect("view menu report must be an array");
                assert!(
                    !view_menu.is_empty(),
                    "clicking the real View button must expose at least one expected menu item: {report:#}"
                );
                for item in view_menu {
                    assert_eq!(
                        item["color"], item["expected"],
                        "view menu colour mismatch: {item:#}"
                    );
                }

                {
                    let screenshot = client
                        .call(
                            "Page.captureScreenshot",
                            json!({ "format": "png", "captureBeyondViewport": false }),
                        )
                        .await?;
                    let encoded = screenshot
                        .pointer("/result/data")
                        .and_then(Value::as_str)
                        .expect("Page.captureScreenshot must return PNG data");
                    let directory = std::env::current_dir()
                        .expect("current workspace directory")
                        .join("artifacts")
                        .join("live-cdp");
                    fs::create_dir_all(&directory).expect("create live CDP screenshot directory");
                    let file = directory.join(format!("{}-{}.png", index + 1, background_id));
                    fs::write(&file, decode_base64(encoded)).expect("write live CDP screenshot");
                    eprintln!("live screenshot: {}", file.display());
                }

                eprintln!("live CDP report: {report:#}");
                reports.push(report);
            }
            Ok::<Vec<Value>, crate::error::CommandError>(reports)
        }
        .await;

        let restored = state.restore_theme().await;
        if let Ok(mut library) = storage::load_theme_library() {
            library.selected_theme_id = None;
            let _ = storage::save_theme_library(&library);
        }

        let reports = verification.expect("live theme verification must succeed");
        let restored = restored.expect("live verification must restore Codex afterward");
        assert!(
            !restored.active,
            "restore must remove the CodeSkin runtime layers after live verification"
        );
        assert_eq!(reports.len(), 3);
    }

    async fn evaluate_live_region_styles(
        client: &CdpClient,
    ) -> Result<Value, crate::error::CommandError> {
        let expression = r#"
(async () => {
  const root = document.documentElement;
  const infoSelector = '[class*="bg-token-dropdown-background"]:has([class~="group/summary-panel-item"])';
  const triggerSelector = 'button.no-drag[aria-haspopup="menu"][class*="text-token-text-tertiary"]';
  const normalise = (value) => {
    const probe = document.createElement('span');
    probe.style.color = value;
    document.body.appendChild(probe);
    const resolved = getComputedStyle(probe).color;
    probe.remove();
    return resolved;
  };
  const contentExpected = normalise(getComputedStyle(root).getPropertyValue('--codeskin-content-foreground').trim());
  const infoExpected = normalise(getComputedStyle(root).getPropertyValue('--codeskin-info-foreground').trim());
  const buttons = [...document.querySelectorAll(triggerSelector)];
  const topFile = buttons.find((node) => /^(文件|File)$/i.test(node.textContent.trim())) || buttons[0] || null;
  const view = buttons.find((node) => /^(视图|View)$/i.test(node.textContent.trim()));
  if (view) {
    view.click();
    await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
    await new Promise((resolve) => setTimeout(resolve, 180));
  }
  const expectedLabels = new Set(['审阅', '终端', '浏览器', '文件', '侧边任务', 'Review', 'Terminal', 'Browser', 'Files', 'Tasks']);
  const viewMenu = [...document.querySelectorAll('.main-surface *')]
    .filter((node) => expectedLabels.has(node.textContent.trim()))
    .map((node) => ({ label: node.textContent.trim(), color: getComputedStyle(node).color, expected: contentExpected }));
  const info = document.querySelector(infoSelector);
  return {
    themeId: root.getAttribute('data-codeskin-theme-id'),
    variables: {
      contentForeground: getComputedStyle(root).getPropertyValue('--codeskin-content-foreground').trim(),
      infoForeground: getComputedStyle(root).getPropertyValue('--codeskin-info-foreground').trim()
    },
    info: info ? { color: getComputedStyle(info).color, expected: infoExpected } : null,
    topFile: topFile ? { label: topFile.textContent.trim(), color: getComputedStyle(topFile).color, expected: contentExpected } : null,
    viewMenu
  };
})()
"#;
        let response = client
            .call(
                "Runtime.evaluate",
                json!({ "expression": expression, "returnByValue": true, "awaitPromise": true }),
            )
            .await?;
        if let Some(error) = response.get("error") {
            return Err(crate::error::CommandError::new(
                "live_cdp_evaluate_failed",
                error.to_string(),
            ));
        }
        response
            .pointer("/result/result/value")
            .cloned()
            .ok_or_else(|| {
                crate::error::CommandError::new("live_cdp_evaluate_failed", response.to_string())
            })
    }

    fn decode_base64(input: &str) -> Vec<u8> {
        fn sextet(byte: u8) -> Option<u8> {
            match byte {
                b'A'..=b'Z' => Some(byte - b'A'),
                b'a'..=b'z' => Some(byte - b'a' + 26),
                b'0'..=b'9' => Some(byte - b'0' + 52),
                b'+' => Some(62),
                b'/' => Some(63),
                _ => None,
            }
        }

        let bytes = input.as_bytes();
        let mut decoded = Vec::with_capacity(bytes.len() * 3 / 4);
        for chunk in bytes.chunks(4) {
            if chunk.len() != 4 {
                break;
            }
            let first = sextet(chunk[0]).expect("valid base64 data");
            let second = sextet(chunk[1]).expect("valid base64 data");
            let third = if chunk[2] == b'=' {
                0
            } else {
                sextet(chunk[2]).expect("valid base64 data")
            };
            let fourth = if chunk[3] == b'=' {
                0
            } else {
                sextet(chunk[3]).expect("valid base64 data")
            };
            decoded.push((first << 2) | (second >> 4));
            if chunk[2] != b'=' {
                decoded.push((second << 4) | (third >> 2));
            }
            if chunk[3] != b'=' {
                decoded.push((third << 6) | fourth);
            }
        }
        decoded
    }

    #[test]
    fn resolves_uploaded_background_id_collisions_with_incrementing_suffixes() {
        let mut library = ThemeLibrary::empty();
        library.themes.push(crate::models::Theme::wallpaper(
            "wallpaper-sunset".into(),
            "Sunset".into(),
            "test".into(),
            crate::models::ThemeColors {
                accent: "#000000".into(),
                secondary: "#8B9DFF".into(),
                background: "#000000".into(),
                surface: "#000000".into(),
                foreground: "#000000".into(),
                muted: "#000000".into(),
            },
            "file:///display.jpg".into(),
            "file:///source.png".into(),
            crate::models::ThemeLayers::wallpaper(),
        ));
        assert_eq!(
            unique_background_id("wallpaper-sunset", &library),
            "wallpaper-sunset-2"
        );
        assert_eq!(
            unique_background_id("wallpaper-river", &library),
            "wallpaper-river"
        );
    }
}
