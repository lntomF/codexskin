use crate::{
    app_state::AppState,
    diagnostic,
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
    diagnostic(format_args!(
        "[save] UI invoke apply_background backgroundId={background_id}"
    ));
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

    diagnostic(format_args!(
        "[apply] selected id={} displayImage={:?} palette=({}, {}, {})",
        background_id,
        background.background_image,
        background.colors.accent,
        background.colors.secondary,
        background.colors.background
    ));
    library.selected_theme_id = Some(background_id.to_owned());
    diagnostic(format_args!(
        "[save] persisting selected background before CDP apply id={background_id}"
    ));
    storage::save_theme_library(&library)?;
    state.apply_saved_theme(background).await
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
        diagnostic(format_args!(
            "[delete] deleting currently selected background id={background_id}; clearing persisted selection"
        ));
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
    diagnostic(
        "[restore] user requested original appearance; clearing persisted selected background",
    );
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
        models::ThemeLibrary,
        process, storage,
    };
    use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
    use serde_json::{json, Value};
    use std::{
        fs,
        io::Cursor,
        time::{Duration, Instant},
    };

    #[tokio::test]
    #[ignore = "requires running Codex Desktop with loopback CDP; creates and removes one temporary cool-light wallpaper"]
    async fn prints_live_header_palette_styles_for_warm_cool_and_dark_wallpapers() {
        const TEMPORARY_COOL_NAME: &str = "CodeSkin header CDP temporary cool-light";
        let state = AppState::new();
        let original_selected = storage::load_theme_library()
            .expect("read original background library")
            .selected_theme_id;

        let verification = async {
            // The local library has a warm light and several dark examples, but no
            // cool light header. Import one temporary CodeSkin-managed wallpaper so
            // the live check exercises all three requested header conditions.
            let imported =
                super::import_background(cool_light_wallpaper_png(), TEMPORARY_COOL_NAME.into())?;
            let cool_id = imported
                .backgrounds
                .iter()
                .find(|background| background.name == TEMPORARY_COOL_NAME)
                .map(|background| background.id.clone())
                .ok_or_else(|| {
                    crate::error::CommandError::new(
                        "live_cdp_temp_wallpaper_missing",
                        "临时冷色浅色壁纸未出现在背景库中。",
                    )
                })?;
            let library = storage::load_theme_library()?;
            let warm_id = library
                .themes
                .iter()
                .find(|theme| theme.id == "wallpaper-81769f62c8016766")
                .or_else(|| library.themes.iter().find(|theme| theme.id != cool_id))
                .map(|theme| theme.id.clone())
                .ok_or_else(|| {
                    crate::error::CommandError::new(
                        "live_cdp_warm_wallpaper_missing",
                        "实时验证需要至少一张已保存的暖色壁纸。",
                    )
                })?;
            let dark_id = library
                .themes
                .iter()
                .find(|theme| theme.id == "wallpaper-d826f8ee81ab6679")
                .or_else(|| {
                    library
                        .themes
                        .iter()
                        .find(|theme| theme.id != warm_id && theme.id != cool_id)
                })
                .map(|theme| theme.id.clone())
                .ok_or_else(|| {
                    crate::error::CommandError::new(
                        "live_cdp_dark_wallpaper_missing",
                        "实时验证需要至少一张已保存的深色壁纸。",
                    )
                })?;
            let backgrounds = [
                ("warm-light", warm_id),
                ("cool-light", cool_id),
                ("dark", dark_id),
            ];

            let mut reports = Vec::new();
            for (index, (kind, background_id)) in backgrounds.iter().enumerate() {
                let applied = apply_saved_background_by_id(background_id, &state).await?;
                assert!(
                    applied.active,
                    "theme {background_id} did not verify as active"
                );
                let status = process::inspect_running_codex();
                let port = status
                    .port
                    .expect("live verification requires detected loopback CDP");
                let target = discover_page_targets(port)
                    .await?
                    .into_iter()
                    .find(|target| target.url.contains("index.html"))
                    .expect("live verification requires an index.html page target");
                let client = CdpClient::connect(&target.websocket_url, port).await?;
                let report = wait_for_live_theme_report(&client, background_id).await?;
                assert_eq!(report["topFile"]["color"], report["topFile"]["expected"]);
                let header_triggers = report["headerMenuTriggers"]
                    .as_array()
                    .expect("header menu trigger array");
                assert_eq!(header_triggers.len(), 4, "expected File/Edit/View/Help");
                for trigger in header_triggers {
                    assert_eq!(trigger["color"], report["headerExpected"]);
                }
                for icon in report["navigationIcons"]
                    .as_array()
                    .expect("navigation icon array")
                {
                    assert_eq!(icon["color"], report["headerIconExpected"]);
                }

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
                let file = directory.join(format!(
                    "header-{}-{}-{}.png",
                    index + 1,
                    kind,
                    background_id
                ));
                fs::write(&file, decode_base64(encoded)).expect("write live CDP screenshot");
                eprintln!("live screenshot: {}", file.display());
                eprintln!("live CDP report: {report:#}");
                reports.push(report);
            }
            Ok::<Vec<Value>, crate::error::CommandError>(reports)
        }
        .await;

        let restore_result = match original_selected.as_deref() {
            Some(background_id) => apply_saved_background_by_id(background_id, &state).await,
            None => state.restore_theme().await,
        };
        let cleanup_result = (|| -> Result<(), crate::error::CommandError> {
            let mut library = storage::load_theme_library()?;
            while let Some(index) = library
                .themes
                .iter()
                .position(|theme| theme.name == TEMPORARY_COOL_NAME)
            {
                let temporary = library.themes.remove(index);
                storage::delete_managed_background_files([
                    temporary.background_image,
                    temporary.source_image,
                ])?;
            }
            library.selected_theme_id = original_selected.clone();
            storage::save_theme_library(&library)
        })();

        let reports = verification.expect("live header verification must succeed");
        let restored = restore_result.expect("live verification must restore original appearance");
        cleanup_result.expect("temporary cool-light wallpaper must be removed");
        assert!(!restored.active || original_selected.is_some());
        assert_eq!(reports.len(), 3);
    }

    fn cool_light_wallpaper_png() -> Vec<u8> {
        let image = RgbImage::from_fn(320, 180, |x, y| {
            let blue = 235_u8.saturating_sub((y / 10) as u8);
            let green = 225_u8.saturating_sub((x / 24) as u8);
            Rgb([198 + (x / 32) as u8, green, blue])
        });
        let mut bytes = Vec::new();
        DynamicImage::ImageRgb8(image)
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .expect("encode temporary cool wallpaper");
        bytes
    }

    async fn wait_for_live_theme_report(
        client: &CdpClient,
        expected_theme_id: &str,
    ) -> Result<Value, crate::error::CommandError> {
        let deadline = Instant::now() + Duration::from_secs(10);

        loop {
            let observation = match evaluate_live_region_styles(client).await {
                Ok(report) if report["themeId"].as_str() == Some(expected_theme_id) => {
                    return Ok(report);
                }
                Ok(report) => format!(
                    "renderer still reports theme {:?}",
                    report["themeId"].as_str()
                ),
                Err(error) => format!("{}: {}", error.code, error.message),
            };

            if Instant::now() >= deadline {
                return Err(crate::error::CommandError::new(
                    "live_header_theme_wait_timeout",
                    format!(
                        "等待 Codex renderer 应用主题 {expected_theme_id} 超时（最后状态：{observation}）。"
                    ),
                ));
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    async fn evaluate_live_region_styles(
        client: &CdpClient,
    ) -> Result<Value, crate::error::CommandError> {
        let expression = r#"
(async () => {
  const root = document.documentElement;
  const infoSelector = '[class*="bg-token-dropdown-background"]:has([class~="group/summary-panel-item"])';
  const triggerSelector = 'button.no-drag[aria-haspopup="menu"][class*="text-token-text-tertiary"]';
  const headerMenuSelector = '.app-header-tint[class*="application-menu-top-bar"] button.no-drag[aria-haspopup="menu"]';
  const normalise = (value) => {
    const probe = document.createElement('span');
    probe.style.color = value;
    document.body.appendChild(probe);
    const resolved = getComputedStyle(probe).color;
    probe.remove();
    return resolved;
  };
  const contentExpected = normalise(getComputedStyle(root).getPropertyValue('--codeskin-content-foreground').trim());
  const headerExpected = normalise(getComputedStyle(root).getPropertyValue('--codeskin-header-foreground').trim());
  const headerIconExpected = normalise(getComputedStyle(root).getPropertyValue('--codeskin-header-icon-foreground').trim());
  const infoExpected = normalise(getComputedStyle(root).getPropertyValue('--codeskin-info-foreground').trim());
  const describe = (node, selector) => node ? {
    selector, matchCount: document.querySelectorAll(selector).length, tag: node.tagName,
    class: node.className, text: node.textContent.trim(), color: getComputedStyle(node).color,
    root: node.getRootNode() === document ? 'document' : (node.getRootNode() instanceof ShadowRoot ? 'shadow-dom' : 'other'),
    iframe: window.top !== window,
    ancestors: (() => { const values=[]; for(let current=node; current && values.length<6; current=current.parentElement){ values.push(`${current.tagName.toLowerCase()}.${String(current.className || '').replace(/\s+/g,'.')}`); } return values; })()
  } : null;
  const buttons = [...document.querySelectorAll(triggerSelector)];
  const headerButtons = [...document.querySelectorAll(headerMenuSelector)];
  const topFile = headerButtons.find((node) => /^(文件|File)$/i.test(node.textContent.trim())) || headerButtons[0] || null;
  const view = headerButtons.find((node) => /^(视图|View)$/i.test(node.textContent.trim()));
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
      headerForeground: getComputedStyle(root).getPropertyValue('--codeskin-header-foreground').trim(),
      headerMutedForeground: getComputedStyle(root).getPropertyValue('--codeskin-header-muted-foreground').trim(),
      contentForeground: getComputedStyle(root).getPropertyValue('--codeskin-content-foreground').trim(),
      infoForeground: getComputedStyle(root).getPropertyValue('--codeskin-info-foreground').trim()
    },
    headerExpected,
    headerIconExpected,
    headerContainer: describe(document.querySelector('.app-header-tint[class*="application-menu-top-bar"]'), '.app-header-tint[class*="application-menu-top-bar"]'),
    topMenuTriggers: buttons.map((node) => describe(node, triggerSelector)),
    headerMenuTriggers: [...document.querySelectorAll(headerMenuSelector)].map((node) => describe(node, headerMenuSelector)),
    info: info ? { color: getComputedStyle(info).color, expected: infoExpected } : null,
    topFile: topFile ? { label: topFile.textContent.trim(), color: getComputedStyle(topFile).color, expected: headerExpected } : null,
    navigationIcons: [...document.querySelectorAll('.app-header-tint button[aria-label]')]
      .filter((node) => /侧边栏|sidebar|back|forward|后退|前进/i.test(node.getAttribute('aria-label') || ''))
      .map((node) => ({ label: node.getAttribute('aria-label'), color: getComputedStyle(node).color })),
    windowControls: ['minimize','maximize','restore','close','最小化','最大化','还原','关闭'].map((label) => ({ label, count: document.querySelectorAll(`[aria-label*="${label}"], [title*="${label}"]`).length })),
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
