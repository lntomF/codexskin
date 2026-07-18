# Task 7 Release Validation Review Package

**日期：2026 年 7 月 17 日（星期五）**  
**工作区：** `G:\FOR STUDY\AI Agent\Electron change codex skin\codeskin`  
**发布程序：** `G:\FOR STUDY\AI Agent\Electron change codex skin\codeskin\src-tauri\target\release\codeskin.exe`

## 范围和约束

- 本次仅修改 `src-tauri/src/app_state.rs` 与本审查/验证文档；未新增依赖、未提交版本控制。
- 未修改 Codex/ChatGPT 官方文件、`app.asar` 或签名。
- CDP 验证只连接已确认的回环地址 `127.0.0.1:49717`；未对外暴露该端口。
- 未停止 ChatGPT/Codex，未执行系统关机。构建前不存在 `codeskin.exe` 输出锁占用者；发布冒烟结束时只停止了本次隐藏启动的 CodeSkin PID `9124`。
- `git rev-parse --is-inside-work-tree` 的退出码为 `128`（不是 Git 仓库），因此没有 commit。

## 本轮 Important 修正：未注册 target 的资源生命周期

### 根因

`install_on_target` 在 `Page.addScriptToEvaluateOnNewDocument` 成功并取得 `identifier` 后，会在将 target 写入运行时 registry 之前调用一次 `Runtime.evaluate`。此前该调用使用 `?`：一旦传输或 CDP 协议执行失败，函数立即返回，既没有 registry 条目供后续 `restore_theme` 清理，也没有本地移除新文档脚本，因此会遗留 new-document 注入。

### 修正

- 把立即执行的 `Runtime.evaluate` 结果先保存为 `execute_result`，不再直接通过 `?` 提前返回。
- 新增本地清理路径 `cleanup_unregistered_target_script`。在该立即执行失败时，无论移除是否成功，都会依次尝试：
  1. `Page.removeScriptToEvaluateOnNewDocument(identifier)`；
  2. 通过 `Runtime.evaluate` 执行 `RESTORE_SCRIPT`。
- 新增可注入 cleanup future 的 `execute_after_script_registration`：它只在执行失败时等待 cleanup，再返回 `install_execute_failed_after_script_registration`。错误消息保留原执行错误 code/message；若任一清理步骤失败，消息明确包含“本地清理失败”及每项失败，绝不静默报告成功。
- 清理发生在任何 watcher 创建、registry 写入或 runtime mutex 获取之前，因而没有新增 watcher 取消或 registry lock 死锁窗口。
- 既有 `restore_theme` 的浏览器实际验证修正保留：每个已注册 target 在 remove、restore 后运行 `VERIFY_SCRIPT`，只有实际值 `active=false`、`wallpaper_layer=false`、`style_layer=false` 且没有 CDP cleanup/verify 错误才成功。

## 回归覆盖

`app_state::live_cdp_tests` 新增两项非 ignored async 单元测试：

1. `install_execute_failure_attempts_local_cleanup_before_returning`：模拟立即执行失败，断言 cleanup closure 已被调用、原始 `execute_failed` code/message 被保留，并说明已尝试移除 new-document 注入和恢复当前页面。
2. `install_execute_failure_reports_local_cleanup_errors`：模拟 remove 与 restore 均失败，断言返回错误明确包含“本地清理失败”和两项清理错误。

测试驱动过程曾先运行缺少 helper 的测试（编译失败），再加入最小 helper；随后加入“保留原始 execute code”的断言并确认其先失败，再修正错误格式。以下是最终、完整验证结果。

## 最终自动验证（本轮实际运行）

| 命令 | 结果 |
|---|---|
| `cargo fmt --manifest-path src-tauri\Cargo.toml -- --check` | 通过。先执行了 `cargo fmt --manifest-path src-tauri\Cargo.toml`，随后 check 退出码 0。 |
| `cargo test --manifest-path src-tauri\Cargo.toml` | 通过：**53 passed, 0 failed, 2 ignored**；doc tests 0。 |
| `cargo check --manifest-path src-tauri\Cargo.toml` | 通过，退出码 0。 |
| `cargo test --manifest-path src-tauri\Cargo.toml live_cdp_tests::applies_and_verifies_a_theme_on_live_codex -- --ignored --nocapture` | 通过：**1 passed, 0 failed, 54 filtered out**。使用已有本地 wallpaper theme，对 `127.0.0.1:49717` 的当前 Codex 页面 apply/verify/restore。 |
| `npm.cmd run build:desktop` | 通过：`tsc && vite build` 通过，`tauri build --no-bundle` 的优化 release build 通过。 |
| 隐藏 release 冒烟期间再次运行上述 ignored live test | 通过：**1 passed, 0 failed, 54 filtered out**。 |

非阻断、且不在允许修改范围内的编译警告保持为：

- `storage::import_background_bytes` 未使用；
- `load_settings` 未使用；
- `save_settings` 未使用。

## 发布与运行时证据

- 产物存在：`G:\FOR STUDY\AI Agent\Electron change codex skin\codeskin\src-tauri\target\release\codeskin.exe`，构建后大小为 `11,373,056` bytes。
- 构建前检测到没有 `codeskin.exe`，故未为释放输出锁停止任何进程。
- 以隐藏窗口启动该 release 产物，PID `9124`；3 秒后确认 `codeskin` 进程仍存活。
- release 进程存活时，端口 `1420` 没有监听者，故本次生产产物运行不依赖开发服务器 `localhost:1420`。
- 冒烟前后均确认 CDP listener 是 `127.0.0.1:49717`（验证时 owner PID `30828`），没有非回环监听。
- 第二次 live test 完成其每 target restore 断言后，只用 `Stop-Process -Id 9124 -Force` 停止该冒烟 CodeSkin 实例；随后确认没有任何 `codeskin.exe` 残留。未操作 ChatGPT/Codex。

## 人工确认边界

没有可靠的 GUI 自动化设备，因此以下项目**未报告为通过，仍需人工确认**：

1. 窗口点击/键盘输入、文件选择器、实际壁纸视觉效果；
2. 托盘图标可见性、菜单/主题选择、Restore 与正常托盘 Quit；
3. 用户触发的刷新与新窗口行为。

CDP live test 证明的是本机浏览器 target 的 apply/verify/restore 合约；没有使用截图冒充上述人工 UI 验收。

## 记录一致性

历史验证脚本的 PowerShell 事件统一称为只读自动变量 `$PID`；本轮未复现该事件。本文及配套验证报告中不存在旧的数值 PID 误称。未创建临时 workspace PID 文件。

未执行系统关机；用户要求的正常关机仍由主代理处理。
