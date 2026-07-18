use crate::{
    cdp::{discover_page_targets, reconnect::RECONNECT_INTERVAL_SECONDS, CdpClient, PageTarget},
    diagnostic,
    error::CommandError,
    injection::{
        install_expression, InjectionRegistry, RegisteredTarget, RESTORE_SCRIPT, VERIFY_SCRIPT,
    },
    models::{
        CodexConnectionState, CodexStatus, TargetVerification, Theme, ThemeLibrary, VerifyResult,
    },
    process, storage,
};
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::{
    sync::{broadcast, oneshot, Mutex},
    time::{sleep, Duration},
};

struct RuntimeState {
    port: Option<u16>,
    executable_path: Option<PathBuf>,
    active_theme: Option<Theme>,
    registry: InjectionRegistry,
    next_registration_id: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PersistedThemeRecovery {
    NoSelection,
    MissingTheme {
        theme_id: String,
    },
    MissingWallpaper {
        theme_id: String,
        wallpaper: Option<String>,
        detail: String,
    },
    Ready(Theme),
}

pub(crate) fn persisted_theme_recovery(library: &ThemeLibrary) -> PersistedThemeRecovery {
    let Some(theme_id) = library.selected_theme_id.as_ref() else {
        return PersistedThemeRecovery::NoSelection;
    };
    let Some(theme) = library.themes.iter().find(|theme| &theme.id == theme_id) else {
        return PersistedThemeRecovery::MissingTheme {
            theme_id: theme_id.clone(),
        };
    };
    let Some(wallpaper) = theme.background_image.as_ref() else {
        return PersistedThemeRecovery::MissingWallpaper {
            theme_id: theme.id.clone(),
            wallpaper: None,
            detail: "背景缺少派生壁纸路径。".into(),
        };
    };
    match storage::read_managed_background_bytes(wallpaper) {
        Ok(_) => PersistedThemeRecovery::Ready(theme.clone()),
        Err(error) => PersistedThemeRecovery::MissingWallpaper {
            theme_id: theme.id.clone(),
            wallpaper: Some(wallpaper.clone()),
            detail: error.message,
        },
    }
}

pub(crate) fn target_apply_plan(
    active_theme: Option<&Theme>,
    targets: &[PageTarget],
    connected_target_ids: &HashSet<String>,
    refresh_existing: bool,
) -> Vec<(PageTarget, Theme)> {
    let Some(theme) = active_theme else {
        return Vec::new();
    };
    targets
        .iter()
        .filter(|target| refresh_existing || !connected_target_ids.contains(&target.id))
        .cloned()
        .map(|target| (target, theme.clone()))
        .collect()
}

pub struct AppState {
    runtime: Mutex<RuntimeState>,
    reconnect_loop_started: AtomicBool,
}

impl AppState {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            runtime: Mutex::new(RuntimeState {
                port: None,
                executable_path: None,
                active_theme: None,
                registry: InjectionRegistry::default(),
                next_registration_id: 1,
            }),
            reconnect_loop_started: AtomicBool::new(false),
        })
    }

    pub async fn connect_or_start_codex(&self) -> Result<CodexStatus, CommandError> {
        let status = process::inspect_running_codex();
        match status.state {
            CodexConnectionState::DebugPortDetected => {
                let port = status
                    .port
                    .ok_or_else(|| CommandError::internal("检测到 CDP 状态但端口为空。"))?;
                let mut runtime = self.runtime.lock().await;
                runtime.port = Some(port);
                if let Some(path) = status.executable_path.as_ref() {
                    runtime.executable_path = Some(PathBuf::from(path));
                }
                Ok(status)
            }
            CodexConnectionState::RunningWithoutDebugPort => {
                if let Some(path) = status.executable_path.as_ref() {
                    self.runtime.lock().await.executable_path = Some(PathBuf::from(path));
                }
                Err(CommandError::new(
                    "codex_running_without_cdp",
                    status.detail,
                ))
            }
            CodexConnectionState::NotRunning => {
                let remembered_executable = self.runtime.lock().await.executable_path.clone();
                let launch_target = match remembered_executable.filter(|path| path.is_file()) {
                    Some(executable) => process::classify_codex_launch_target(executable)?,
                    None => process::find_installed_codex_launch_target()?.ok_or_else(|| {
                        CommandError::new(
                            "codex_executable_not_found",
                            "未找到 Codex Desktop 可执行文件。请确认 Codex Desktop 已安装。",
                        )
                    })?,
                };
                let port = process::find_available_loopback_port()?;
                process::launch_codex(&launch_target, port)?;
                self.wait_for_page_targets(port).await?;
                self.runtime.lock().await.port = Some(port);
                Ok(CodexStatus {
                    state: CodexConnectionState::Connected,
                    port: Some(port),
                    executable_path: Some(launch_target.executable_path().display().to_string()),
                    detail: "已启动 Codex 并发现本地 CDP 页面。".into(),
                })
            }
            _ => Ok(status),
        }
    }

    pub async fn publish_active_theme(&self, theme: Theme) {
        diagnostic(format_args!(
            "[runtime] publish active_theme={} wallpaper={:?}",
            theme.id, theme.background_image
        ));
        self.runtime.lock().await.active_theme = Some(theme);
    }

    pub async fn restore_persisted_theme_on_startup(self: &Arc<Self>) {
        let path = match storage::theme_library_path() {
            Ok(path) => path,
            Err(error) => {
                diagnostic(format_args!(
                    "[startup] could not resolve theme path: {error}"
                ));
                return;
            }
        };
        let library = match storage::load_theme_library() {
            Ok(library) => library,
            Err(error) => {
                diagnostic(format_args!(
                    "[startup] persisted theme read failed path={}: {error}",
                    path.display()
                ));
                return;
            }
        };
        match persisted_theme_recovery(&library) {
            PersistedThemeRecovery::Ready(theme) => {
                diagnostic(format_args!(
                    "[startup] hydrated selectedThemeId={} from path={}",
                    theme.id,
                    path.display()
                ));
                self.publish_active_theme(theme.clone()).await;
                self.start_reconnector();
                self.apply_published_theme_to_running_codex(theme).await;
            }
            PersistedThemeRecovery::NoSelection => diagnostic(format_args!(
                "[startup] no selected theme in path={}",
                path.display()
            )),
            PersistedThemeRecovery::MissingTheme { theme_id } => diagnostic(format_args!(
                "[startup] selectedThemeId={} is absent from path={}; preserving selection without injection",
                theme_id,
                path.display()
            )),
            PersistedThemeRecovery::MissingWallpaper {
                theme_id,
                wallpaper,
                detail,
            } => diagnostic(format_args!(
                "[startup] selectedThemeId={} wallpaper={:?} is unavailable in path={}; preserving selection without injection: {}",
                theme_id,
                wallpaper,
                path.display(),
                detail
            )),
        }
    }

    async fn apply_published_theme_to_running_codex(self: &Arc<Self>, theme: Theme) {
        let status = process::inspect_running_codex();
        let CodexConnectionState::DebugPortDetected = status.state else {
            diagnostic(format_args!(
                "[startup] no attachable Codex target yet; watcher will wait: state={:?} detail={}",
                status.state, status.detail
            ));
            return;
        };
        let Some(port) = status.port else {
            diagnostic("[startup] attachable Codex status did not provide a CDP port.");
            return;
        };
        self.runtime.lock().await.port = Some(port);
        match self.install_on_all_targets(port, theme, true).await {
            Ok(()) => diagnostic(format_args!(
                "[startup] immediate apply completed for existing Codex port={port}"
            )),
            Err(error) => diagnostic(format_args!(
                "[startup] immediate apply deferred for port={port}: {error}"
            )),
        }
    }

    pub async fn apply_saved_theme(
        self: &Arc<Self>,
        theme: Theme,
    ) -> Result<VerifyResult, CommandError> {
        diagnostic(format_args!(
            "[apply] begin themeId={} wallpaper={:?} palette=({}, {}, {})",
            theme.id,
            theme.background_image,
            theme.colors.accent,
            theme.colors.secondary,
            theme.colors.background
        ));
        self.publish_active_theme(theme.clone()).await;
        self.start_reconnector();
        let status = self.connect_or_start_codex().await?;
        diagnostic(format_args!(
            "[apply] connection status state={:?} port={:?} detail={}",
            status.state, status.port, status.detail
        ));
        let port = status
            .port
            .ok_or_else(|| CommandError::new("cdp_port_missing", "没有可用的本地 CDP 端口。"))?;
        self.install_on_all_targets(port, theme.clone(), true)
            .await?;
        diagnostic(format_args!(
            "[apply] target install completed for active_theme={}",
            theme.id
        ));
        self.verify_theme().await
    }

    pub async fn verify_theme(&self) -> Result<VerifyResult, CommandError> {
        let (theme_id, targets) = {
            let runtime = self.runtime.lock().await;
            (
                runtime.active_theme.as_ref().map(|theme| theme.id.clone()),
                runtime
                    .registry
                    .targets
                    .values()
                    .map(|target| {
                        (
                            target.target_id.clone(),
                            target.target_url.clone(),
                            Arc::clone(&target.client),
                        )
                    })
                    .collect::<Vec<_>>(),
            )
        };

        let mut checks = Vec::with_capacity(targets.len());
        for (target_id, target_url, client) in targets {
            let check = match client
                .call(
                    "Runtime.evaluate",
                    json!({
                        "expression": VERIFY_SCRIPT,
                        "returnByValue": true,
                        "awaitPromise": true
                    }),
                )
                .await
            {
                Ok(response) => match cdp_result(response) {
                    Ok(result) => {
                        let value = result
                            .get("result")
                            .and_then(|value| value.get("value"))
                            .unwrap_or(&Value::Null);
                        diagnostic(format_args!(
                            "[verify] targetId={} targetUrl={} Runtime.evaluate value={}",
                            target_id, target_url, value
                        ));
                        TargetVerification::from_browser_value(target_id, target_url, value)
                    }
                    Err(error) => TargetVerification::failed(target_id, target_url, error.message),
                },
                Err(error) => TargetVerification::failed(target_id, target_url, error.message),
            };
            checks.push(check);
        }

        Ok(VerifyResult {
            theme_id,
            active: !checks.is_empty() && checks.iter().all(|check| check.active),
            targets: checks,
        })
    }

    pub async fn restore_theme(&self) -> Result<VerifyResult, CommandError> {
        let targets = {
            let mut runtime = self.runtime.lock().await;
            runtime.active_theme = None;
            std::mem::take(&mut runtime.registry.targets)
                .into_values()
                .collect::<Vec<_>>()
        };

        let mut checks = Vec::with_capacity(targets.len());
        let mut incomplete_targets = Vec::new();
        for target in targets {
            let _ = target.reload_watcher_stop.send(());
            let target_id = target.target_id;
            let target_url = target.target_url;
            let mut cleanup_errors = Vec::new();

            if let Err(error) = target
                .client
                .call(
                    "Page.removeScriptToEvaluateOnNewDocument",
                    json!({
                        "identifier": target.new_document_script_id
                    }),
                )
                .await
                .and_then(cdp_result)
            {
                cleanup_errors.push(format!("取消刷新注入失败：{}", error.message));
            }
            if let Err(error) = target
                .client
                .call(
                    "Runtime.evaluate",
                    json!({
                        "expression": RESTORE_SCRIPT,
                        "returnByValue": true,
                        "awaitPromise": true
                    }),
                )
                .await
                .and_then(cdp_result)
            {
                cleanup_errors.push(format!("清理已加载页面失败：{}", error.message));
            }

            let (check, post_restore_verify_succeeded) = match target
                .client
                .call(
                    "Runtime.evaluate",
                    json!({
                        "expression": VERIFY_SCRIPT,
                        "returnByValue": true,
                        "awaitPromise": true
                    }),
                )
                .await
            {
                Ok(response) => match cdp_result(response) {
                    Ok(result) => {
                        let value = result
                            .get("result")
                            .and_then(|value| value.get("value"))
                            .unwrap_or(&Value::Null);
                        (
                            TargetVerification::from_browser_value(
                                target_id.clone(),
                                target_url.clone(),
                                value,
                            ),
                            true,
                        )
                    }
                    Err(error) => (
                        TargetVerification::failed(
                            target_id.clone(),
                            target_url.clone(),
                            format!("恢复后浏览器验证失败：{}", error.message),
                        ),
                        false,
                    ),
                },
                Err(error) => (
                    TargetVerification::failed(
                        target_id.clone(),
                        target_url.clone(),
                        format!("恢复后浏览器验证失败：{}", error.message),
                    ),
                    false,
                ),
            };

            if !restore_target_is_complete(&check, post_restore_verify_succeeded, &cleanup_errors) {
                let mut reasons = cleanup_errors;
                if !post_restore_verify_succeeded {
                    reasons.push(check.detail.clone());
                } else if check.active || check.wallpaper_layer || check.style_layer {
                    reasons.push(format!("恢复后仍检测到 CodeSkin marker：{}", check.detail));
                }
                incomplete_targets.push(format!("{}: {}", check.target_id, reasons.join("；")));
            }
            checks.push(check);
        }

        let result = VerifyResult {
            theme_id: None,
            active: false,
            targets: checks,
        };
        if incomplete_targets.is_empty() {
            Ok(result)
        } else {
            Err(CommandError::new(
                "restore_incomplete",
                format!(
                    "CodeSkin 清理未完成；已尝试所有已注册 target：{}",
                    incomplete_targets.join(" | ")
                ),
            ))
        }
    }

    pub fn start_reconnector(self: &Arc<Self>) {
        if self.reconnect_loop_started.swap(true, Ordering::AcqRel) {
            return;
        }
        diagnostic("[watcher] reconnect loop started.");
        let state = Arc::clone(self);
        tokio::spawn(async move {
            loop {
                sleep(Duration::from_secs(RECONNECT_INTERVAL_SECONDS)).await;
                let theme = state.runtime.lock().await.active_theme.clone();
                let Some(theme) = theme else {
                    diagnostic("[watcher] no runtime active_theme; skipping Codex discovery.");
                    continue;
                };

                let status = process::inspect_running_codex();
                diagnostic(format_args!(
                    "[watcher] Codex inspect state={:?} port={:?} executable={:?} detail={}",
                    status.state, status.port, status.executable_path, status.detail
                ));
                let CodexConnectionState::DebugPortDetected = status.state else {
                    continue;
                };
                let Some(port) = status.port else { continue };

                state.runtime.lock().await.port = Some(port);
                match state.install_on_all_targets(port, theme, false).await {
                    Ok(()) => diagnostic(format_args!("[watcher] apply completed for port={port}")),
                    Err(error) => diagnostic(format_args!(
                        "[watcher] apply failed for port={port}: {error}"
                    )),
                };
            }
        });
    }

    async fn wait_for_page_targets(&self, port: u16) -> Result<(), CommandError> {
        let mut last_error = None;
        for _ in 0..50 {
            match discover_page_targets(port).await {
                Ok(targets) if !targets.is_empty() => return Ok(()),
                Ok(_) => last_error = Some("Codex 已启动，但尚未出现页面 target。".to_string()),
                Err(error) => last_error = Some(error.message),
            }
            sleep(Duration::from_millis(300)).await;
        }
        Err(cdp_start_timeout_error(port, last_error))
    }

    async fn install_on_all_targets(
        self: &Arc<Self>,
        port: u16,
        theme: Theme,
        refresh_existing: bool,
    ) -> Result<(), CommandError> {
        let targets = discover_page_targets(port).await?;
        diagnostic(format_args!(
            "[watcher] discover targets port={} refreshExisting={} targets={:?}",
            port,
            refresh_existing,
            targets
                .iter()
                .map(|target| (&target.id, &target.url, &target.websocket_url))
                .collect::<Vec<_>>()
        ));
        if targets.is_empty() {
            return Err(CommandError::new(
                "codex_page_not_found",
                "本地 CDP 未返回可注入的 page target。",
            ));
        }

        let live_ids = targets
            .iter()
            .map(|target| target.id.clone())
            .collect::<HashSet<_>>();
        let stale_targets = {
            let mut runtime = self.runtime.lock().await;
            let stale_ids = runtime
                .registry
                .targets
                .keys()
                .filter(|id| !live_ids.contains(*id))
                .cloned()
                .collect::<Vec<_>>();
            stale_ids
                .into_iter()
                .filter_map(|id| runtime.registry.targets.remove(&id))
                .collect::<Vec<_>>()
        };
        drop(stale_targets);

        let connected_target_ids = {
            let runtime = self.runtime.lock().await;
            runtime
                .registry
                .targets
                .iter()
                .filter_map(|(id, registered)| registered.client.is_connected().then(|| id.clone()))
                .collect::<HashSet<_>>()
        };
        for (target, target_theme) in target_apply_plan(
            Some(&theme),
            &targets,
            &connected_target_ids,
            refresh_existing,
        ) {
            self.install_on_target(port, target, target_theme).await?;
        }
        Ok(())
    }

    async fn install_on_target(
        self: &Arc<Self>,
        port: u16,
        target: PageTarget,
        theme: Theme,
    ) -> Result<(), CommandError> {
        let previous = self
            .runtime
            .lock()
            .await
            .registry
            .targets
            .remove(&target.id);
        if let Some(previous) = previous {
            let _ = previous.reload_watcher_stop.send(());
            let _ = previous
                .client
                .call(
                    "Page.removeScriptToEvaluateOnNewDocument",
                    json!({
                        "identifier": previous.new_document_script_id
                    }),
                )
                .await;
        }

        diagnostic(format_args!(
            "[apply] installing targetId={} targetUrl={} websocket={}",
            target.id, target.url, target.websocket_url
        ));
        let client = Arc::new(CdpClient::connect(&target.websocket_url, port).await?);
        let event_receiver = client.subscribe_events();
        cdp_result(client.call("Page.enable", json!({})).await?)?;

        let expression = install_expression(&theme)?;
        let add_script_response = cdp_result(
            client
                .call(
                    "Page.addScriptToEvaluateOnNewDocument",
                    json!({
                        "source": expression
                    }),
                )
                .await?,
        )?;
        let script_id = add_script_response
            .get("identifier")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                CommandError::new(
                    "invalid_cdp_add_script_response",
                    "CDP 未返回 new-document script identifier。",
                )
            })?
            .to_string();

        let execute_result = match client
            .call(
                "Runtime.evaluate",
                json!({
                    "expression": expression,
                    "returnByValue": true,
                    "awaitPromise": true
                }),
            )
            .await
        {
            Ok(response) => cdp_result(response),
            Err(error) => Err(error),
        };
        match &execute_result {
            Ok(value) => diagnostic(format_args!(
                "[cdp] Runtime.evaluate install targetId={} success result={}",
                target.id, value
            )),
            Err(error) => diagnostic(format_args!(
                "[cdp] Runtime.evaluate install targetId={} error={error}",
                target.id
            )),
        }
        let cleanup_client = Arc::clone(&client);
        let cleanup_script_id = script_id.clone();
        execute_after_script_registration(execute_result, move || async move {
            cleanup_unregistered_target_script(&cleanup_client, &cleanup_script_id).await
        })
        .await?;

        let (reload_watcher_stop, reload_watcher_shutdown) = oneshot::channel();
        let target_id = target.id;
        let target_url = target.url;
        let registration_id = {
            let mut runtime = self.runtime.lock().await;
            let registration_id = runtime.next_registration_id;
            runtime.next_registration_id = runtime.next_registration_id.wrapping_add(1).max(1);
            runtime.registry.targets.insert(
                target_id.clone(),
                RegisteredTarget {
                    target_id: target_id.clone(),
                    target_url,
                    client: Arc::clone(&client),
                    new_document_script_id: script_id,
                    registration_id,
                    reload_watcher_stop,
                },
            );
            registration_id
        };

        let state = Arc::clone(self);
        tokio::spawn(async move {
            state
                .watch_target_load_events(
                    target_id,
                    registration_id,
                    client,
                    event_receiver,
                    reload_watcher_shutdown,
                )
                .await;
        });
        Ok(())
    }

    async fn watch_target_load_events(
        self: Arc<Self>,
        target_id: String,
        registration_id: u64,
        client: Arc<CdpClient>,
        mut events: broadcast::Receiver<crate::cdp::CdpEvent>,
        mut shutdown: oneshot::Receiver<()>,
    ) {
        loop {
            let event = tokio::select! {
                _ = &mut shutdown => return,
                event = events.recv() => event,
            };
            let event = match event {
                Ok(event) => event,
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => return,
            };
            if event.method != "Page.loadEventFired" || !event.params.is_object() {
                continue;
            }

            tokio::select! {
                _ = &mut shutdown => return,
                _ = sleep(Duration::from_millis(250)) => {}
            }

            let theme = {
                let runtime = self.runtime.lock().await;
                let is_current_registration =
                    runtime
                        .registry
                        .targets
                        .get(&target_id)
                        .is_some_and(|registered| {
                            registered.registration_id == registration_id
                                && Arc::ptr_eq(&registered.client, &client)
                        });
                if !is_current_registration {
                    return;
                }
                runtime.active_theme.clone()
            };
            let Some(theme) = theme else {
                // `apply_theme` registers the watcher before publishing the active theme.
                // Keep the subscription alive across that small setup window; restore cancels it.
                continue;
            };
            let expression = match install_expression(&theme) {
                Ok(expression) => expression,
                Err(error) => {
                    eprintln!(
                        "CodeSkin could not rebuild wallpaper injection for {target_id}: {error}"
                    );
                    continue;
                }
            };

            let _ = client
                .call(
                    "Runtime.evaluate",
                    json!({
                        "expression": expression,
                        "returnByValue": true,
                        "awaitPromise": true
                    }),
                )
                .await
                .and_then(cdp_result);
        }
    }
}

async fn cleanup_unregistered_target_script(client: &CdpClient, script_id: &str) -> Vec<String> {
    let mut cleanup_errors = Vec::new();

    if let Err(error) = client
        .call(
            "Page.removeScriptToEvaluateOnNewDocument",
            json!({
                "identifier": script_id
            }),
        )
        .await
        .and_then(cdp_result)
    {
        cleanup_errors.push(format!("取消刷新注入失败：{}", error.message));
    }
    if let Err(error) = client
        .call(
            "Runtime.evaluate",
            json!({
                "expression": RESTORE_SCRIPT,
                "returnByValue": true,
                "awaitPromise": true
            }),
        )
        .await
        .and_then(cdp_result)
    {
        cleanup_errors.push(format!("清理已加载页面失败：{}", error.message));
    }

    cleanup_errors
}

async fn execute_after_script_registration<T, Cleanup, CleanupFuture>(
    execute_result: Result<T, CommandError>,
    cleanup: Cleanup,
) -> Result<T, CommandError>
where
    Cleanup: FnOnce() -> CleanupFuture,
    CleanupFuture: std::future::Future<Output = Vec<String>>,
{
    match execute_result {
        Ok(value) => Ok(value),
        Err(execute_error) => {
            let cleanup_errors = cleanup().await;
            let cleanup_detail = if cleanup_errors.is_empty() {
                "已尝试取消 new-document 注入并恢复当前页面。".to_string()
            } else {
                format!("本地清理失败：{}", cleanup_errors.join("；"))
            };
            Err(CommandError::new(
                "install_execute_failed_after_script_registration",
                format!(
                    "new-document script 注册后立即注入失败（{}）：{}；{}",
                    execute_error.code, execute_error.message, cleanup_detail
                ),
            ))
        }
    }
}

fn cdp_start_timeout_error(port: u16, last_error: Option<String>) -> CommandError {
    let last_check = last_error.unwrap_or_else(|| "等待 Codex 本地 CDP 超时。".into());
    CommandError::new(
        "codex_cdp_start_timeout",
        format!(
            "Codex 已收到启动请求，但在限定时间内未在 127.0.0.1:{port} 提供可注入的 CDP 页面。\n\
             未连接任何外部地址。请先通过 Codex 自己的菜单或托盘正常退出，并确认进程已完全结束后重试。\n\
             如果这是 Microsoft Store 版 Codex，当前版本可能没有将启动参数转交给 Electron；CodeSkin 不会强制结束 Codex，也不会修改其安装文件。\n\
             最后一次本机检查：{last_check}"
        ),
    )
}
fn restore_target_is_complete(
    check: &TargetVerification,
    post_restore_verify_succeeded: bool,
    cleanup_errors: &[String],
) -> bool {
    post_restore_verify_succeeded
        && cleanup_errors.is_empty()
        && !check.active
        && !check.wallpaper_layer
        && !check.style_layer
}

fn cdp_result(response: Value) -> Result<Value, CommandError> {
    if let Some(error) = response.get("error") {
        return Err(CommandError::new("cdp_protocol_error", error.to_string()));
    }
    response
        .get("result")
        .cloned()
        .ok_or_else(|| CommandError::new("invalid_cdp_response", "CDP 响应缺少 result 字段。"))
}

#[cfg(test)]
mod live_cdp_tests {
    use super::{
        execute_after_script_registration, persisted_theme_recovery, restore_target_is_complete,
        target_apply_plan, AppState, PersistedThemeRecovery,
    };
    use crate::cdp::PageTarget;
    use crate::error::CommandError;
    use crate::models::{
        TargetVerification, Theme, ThemeColors, ThemeLayers, ThemeLibrary, THEME_LIBRARY_VERSION,
    };
    use std::{
        collections::HashSet,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
    };

    fn wallpaper(id: &str, image: &str) -> Theme {
        Theme::wallpaper(
            id.into(),
            id.into(),
            "test".into(),
            ThemeColors {
                accent: "#123456".into(),
                secondary: "#445566".into(),
                background: "#112233".into(),
                surface: "#223344".into(),
                foreground: "#F4F7FF".into(),
                muted: "#BBC5D8".into(),
            },
            image.into(),
            image.into(),
            ThemeLayers::wallpaper(),
        )
    }

    fn target(id: &str) -> PageTarget {
        PageTarget {
            id: id.into(),
            url: "app://-/index.html".into(),
            websocket_url: format!("ws://127.0.0.1:9222/devtools/page/{id}"),
        }
    }

    #[test]
    fn startup_recovery_preserves_selected_theme_when_wallpaper_file_is_missing() {
        let theme = wallpaper(
            "wallpaper-missing",
            "file:///C:/CodeSkin/wallpapers/does-not-exist.jpg",
        );
        let library = ThemeLibrary {
            version: THEME_LIBRARY_VERSION,
            selected_theme_id: Some(theme.id.clone()),
            themes: vec![theme],
        };

        let recovery = persisted_theme_recovery(&library);

        assert!(matches!(
            recovery,
            PersistedThemeRecovery::MissingWallpaper { ref theme_id, .. }
                if theme_id == "wallpaper-missing"
        ));
        assert_eq!(
            library.selected_theme_id.as_deref(),
            Some("wallpaper-missing")
        );
    }

    #[test]
    fn new_cdp_target_receives_the_current_active_theme_after_old_target_disappears() {
        let theme_b = wallpaper("wallpaper-b", "file:///C:/CodeSkin/wallpapers/b.jpg");
        let connected = HashSet::from(["old-target".to_owned()]);

        let plan = target_apply_plan(Some(&theme_b), &[target("new-target")], &connected, false);

        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].0.id, "new-target");
        assert_eq!(plan[0].1.id, "wallpaper-b");
    }

    #[test]
    fn reconnect_uses_newly_selected_theme_instead_of_a_stale_theme() {
        let theme_a = wallpaper("wallpaper-a", "file:///C:/CodeSkin/wallpapers/a.jpg");
        let theme_b = wallpaper("wallpaper-b", "file:///C:/CodeSkin/wallpapers/b.jpg");
        let connected = HashSet::new();

        let before_user_change =
            target_apply_plan(Some(&theme_a), &[target("target-1")], &connected, false);
        let after_user_change =
            target_apply_plan(Some(&theme_b), &[target("target-2")], &connected, false);

        assert_eq!(before_user_change[0].1.id, "wallpaper-a");
        assert_eq!(after_user_change[0].1.id, "wallpaper-b");
    }

    fn target_check(active: bool, wallpaper_layer: bool, style_layer: bool) -> TargetVerification {
        TargetVerification {
            target_id: "test-target".into(),
            target_url: "app://-/index.html".into(),
            active,
            detail: "test".into(),
            wallpaper_layer,
            wallpaper_configured: false,
            style_layer,
            mode: None,
        }
    }

    #[test]
    fn cdp_start_timeout_explains_loopback_scope_and_store_retry_path() {
        let error = super::cdp_start_timeout_error(9223, Some("connection refused".to_string()));

        assert_eq!(error.code, "codex_cdp_start_timeout");
        assert!(error.message.contains("127.0.0.1:9223"));
        assert!(error.message.contains("Microsoft Store"));
        assert!(error.message.contains("正常退出"));
        assert!(error.message.contains("未连接任何外部地址"));
        assert!(!error.message.contains("0.0.0.0"));
    }
    #[test]
    fn restore_requires_successful_post_browser_verification_and_no_remaining_markers() {
        assert!(restore_target_is_complete(
            &target_check(false, false, false),
            true,
            &[]
        ));
        assert!(!restore_target_is_complete(
            &target_check(false, true, false),
            true,
            &[]
        ));
        assert!(!restore_target_is_complete(
            &target_check(false, false, false),
            false,
            &[]
        ));
        assert!(!restore_target_is_complete(
            &target_check(false, false, false),
            true,
            &["restore eval failed".into()]
        ));
    }

    #[tokio::test]
    async fn install_execute_failure_attempts_local_cleanup_before_returning() {
        let cleanup_attempted = Arc::new(AtomicBool::new(false));
        let cleanup_attempted_for_closure = Arc::clone(&cleanup_attempted);

        let error = execute_after_script_registration::<(), _, _>(
            Err(CommandError::new(
                "execute_failed",
                "runtime evaluate failed",
            )),
            move || async move {
                cleanup_attempted_for_closure.store(true, Ordering::Release);
                Vec::new()
            },
        )
        .await
        .expect_err("an immediate execution failure should be returned after cleanup");

        assert!(cleanup_attempted.load(Ordering::Acquire));
        assert_eq!(
            error.code,
            "install_execute_failed_after_script_registration"
        );
        assert!(error.message.contains("execute_failed"));
        assert!(error.message.contains("runtime evaluate failed"));
        assert!(error
            .message
            .contains("已尝试取消 new-document 注入并恢复当前页面"));
    }

    #[tokio::test]
    async fn install_execute_failure_reports_local_cleanup_errors() {
        let error = execute_after_script_registration::<(), _, _>(
            Err(CommandError::new(
                "execute_failed",
                "runtime evaluate failed",
            )),
            || async {
                vec![
                    "取消刷新注入失败：remove failed".to_string(),
                    "清理已加载页面失败：restore failed".to_string(),
                ]
            },
        )
        .await
        .expect_err("cleanup errors must not be silent");

        assert!(error.message.contains("本地清理失败"));
        assert!(error.message.contains("取消刷新注入失败：remove failed"));
        assert!(error.message.contains("清理已加载页面失败：restore failed"));
    }

    #[tokio::test]
    #[ignore = "requires a running Codex Desktop instance with loopback CDP enabled and an existing local wallpaper theme"]
    async fn applies_and_verifies_a_theme_on_live_codex() {
        let app_data_dir = std::env::var_os("LOCALAPPDATA")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("CodeSkin");
        let theme = std::fs::read(app_data_dir.join("themes.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<crate::models::ThemeLibrary>(&bytes).ok())
            .and_then(|library| {
                library.themes.into_iter().find(|theme| {
                    theme.source == crate::models::ThemeSource::Wallpaper
                        && theme.background_image.is_some()
                })
            })
            .expect("live test requires an existing local CodeSkin wallpaper theme");
        assert_eq!(theme.source, crate::models::ThemeSource::Wallpaper);
        assert!(theme.background_image.is_some());

        let state = AppState::new();
        let apply_result = state.apply_saved_theme(theme).await;
        // Restore before asserting apply results so assertion failures never leave live CodeSkin layers behind.
        let restore_result = state.restore_theme().await;

        let result =
            apply_result.expect("apply_theme should complete against the discovered local target");
        assert!(result.active, "verification result: {result:#?}");
        assert!(
            result.targets.iter().all(|target| target.wallpaper_layer
                && target.style_layer
                && matches!(target.mode.as_deref(), Some("ambient") | Some("focus"))),
            "expected CodeSkin-owned layers and a valid mode: {result:#?}"
        );
        let restored = restore_result.expect("restore_theme should complete");
        assert!(!restored.active, "restore result: {restored:#?}");
        assert!(
            !restored.targets.is_empty()
                && restored.targets.iter().all(|target| {
                    !target.active && !target.wallpaper_layer && !target.style_layer
                }),
            "expected every restored target to be free of CodeSkin markers: {restored:#?}"
        );
    }
}
