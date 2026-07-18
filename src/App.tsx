import { ChangeEvent, useEffect, useRef, useState } from "react";
import {
  applyBackground,
  connectOrStartCodex,
  deleteBackground,
  importBackground,
  inspectCodexStatus,
  loadBackgroundLibrary,
  restoreOriginalAppearance,
  verifyInjection,
} from "./api";
import type {
  Background,
  BackgroundLibrary,
  CodexConnectionState,
  CodexStatus,
  CommandError,
  VerifyResult,
} from "./types";
import "./App.css";

const initialStatus: CodexStatus = {
  state: "notRunning",
  port: null,
  executablePath: null,
  detail: "正在检查 Codex 状态…",
};

const stateLabel: Record<CodexConnectionState, string> = {
  notRunning: "未运行",
  runningWithoutDebugPort: "已运行，未开启 CDP",
  debugPortDetected: "已发现本地 CDP",
  starting: "正在启动",
  connecting: "正在连接",
  connected: "已连接",
  reconnecting: "正在重连",
  error: "连接错误",
};

function isCommandError(value: unknown): value is CommandError {
  return Boolean(
    value &&
      typeof value === "object" &&
      "message" in value &&
      typeof (value as CommandError).message === "string",
  );
}

function errorMessage(error: unknown): string {
  if (isCommandError(error)) return error.message;
  if (typeof error === "string") {
    try {
      const parsed = JSON.parse(error) as unknown;
      if (isCommandError(parsed)) return parsed.message;
    } catch {
      // Tauri may return a plain string for command errors.
    }
    return error;
  }
  return error instanceof Error ? error.message : "发生未知错误。";
}

function describeVerification(result: VerifyResult): string {
  const count = result.targets.length;
  if (result.active) {
    return `背景注入已生效：${count} 个 Codex 渲染页面已校验。`;
  }
  return count
    ? "已连接到 Codex，但没有检测到完整的背景注入层。"
    : "未发现可校验的 Codex 渲染页面。";
}

function App() {
  const [library, setLibrary] = useState<BackgroundLibrary | null>(null);
  const [status, setStatus] = useState<CodexStatus>(initialStatus);
  const [verification, setVerification] = useState<VerifyResult | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [notice, setNotice] = useState("");
  const [error, setError] = useState("");
  const fileInput = useRef<HTMLInputElement>(null);

  const isBusy = busy !== null;

  useEffect(() => {
    void initialize();
  }, []);

  async function initialize() {
    setBusy("initialize");
    setError("");
    try {
      const [nextLibrary, codexStatus] = await Promise.all([
        loadBackgroundLibrary(),
        inspectCodexStatus(),
      ]);
      setLibrary(nextLibrary);
      setStatus(codexStatus);
    } catch (cause) {
      setError(`初始化失败：${errorMessage(cause)}`);
    } finally {
      setBusy(null);
    }
  }

  async function refreshStatus() {
    setBusy("status");
    try {
      setStatus(await inspectCodexStatus());
      setError("");
    } catch (cause) {
      setError(`状态检测失败：${errorMessage(cause)}`);
    } finally {
      setBusy(null);
    }
  }

  async function connectCodex() {
    setBusy("connect");
    setError("");
    setNotice("");
    try {
      const nextStatus = await connectOrStartCodex();
      setStatus(nextStatus);
      setNotice(
        nextStatus.port
          ? `Codex 已连接到本机 127.0.0.1:${nextStatus.port}。`
          : nextStatus.detail,
      );
    } catch (cause) {
      await refreshStatus();
      setError(errorMessage(cause));
    } finally {
      setBusy(null);
    }
  }

  async function apply(background: Background) {
    setBusy(`apply:${background.id}`);
    setError("");
    setNotice("");
    try {
      const result = await applyBackground(background.id);
      setVerification(result);
      setLibrary((current) =>
        current ? { ...current, selectedBackgroundId: background.id } : current,
      );
      setStatus(await inspectCodexStatus());
      setNotice(`“${background.name}”已应用。${describeVerification(result)}`);
    } catch (cause) {
      setError(`无法应用背景：${errorMessage(cause)}`);
      await refreshStatus();
    } finally {
      setBusy(null);
    }
  }

  async function uploadFile(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file) return;

    setBusy("upload");
    setError("");
    setNotice("");
    try {
      const bytes = Array.from(new Uint8Array(await file.arrayBuffer()));
      const nextLibrary = await importBackground(bytes, file.name);
      setLibrary(nextLibrary);
      setNotice(`已保存“${file.name}”。点击它的缩略图即可应用到 Codex。`);
    } catch (cause) {
      setError(`导入失败：${errorMessage(cause)}`);
    } finally {
      setBusy(null);
    }
  }

  async function remove(background: Background) {
    if (
      !window.confirm(
        background.id === library?.selectedBackgroundId
          ? `删除“${background.name}”会先恢复 Codex 原始外观，确定继续吗？`
          : `确定删除“${background.name}”吗？`,
      )
    ) {
      return;
    }

    setBusy(`delete:${background.id}`);
    setError("");
    setNotice("");
    try {
      const nextLibrary = await deleteBackground(background.id);
      setLibrary(nextLibrary);
      setVerification(null);
      setNotice(
        background.id === library?.selectedBackgroundId
          ? "已恢复 Codex 原始外观，并删除这张背景图。"
          : "背景图已删除。",
      );
    } catch (cause) {
      setError(`删除失败：${errorMessage(cause)}`);
    } finally {
      setBusy(null);
    }
  }

  async function verify() {
    setBusy("verify");
    setError("");
    setNotice("");
    try {
      const result = await verifyInjection();
      setVerification(result);
      setNotice(describeVerification(result));
    } catch (cause) {
      setError(`校验失败：${errorMessage(cause)}`);
    } finally {
      setBusy(null);
    }
  }

  async function restore() {
    setBusy("restore");
    setError("");
    setNotice("");
    try {
      const result = await restoreOriginalAppearance();
      setVerification(result);
      setLibrary((current) =>
        current ? { ...current, selectedBackgroundId: null } : current,
      );
      setNotice("已清除 CodeSkin 注入层，Codex 已恢复原始外观。");
    } catch (cause) {
      setError(`恢复失败：${errorMessage(cause)}`);
    } finally {
      setBusy(null);
    }
  }

  return (
    <main className="app-shell">
      <header className="hero">
        <div>
          <p className="eyebrow">CodeSkin · 非官方本地工具</p>
          <h1>我的 Codex 背景</h1>
          <p className="hero-copy">
            上传一张图片，点击缩略图即可作为 Codex 桌面版的背景。所有连接仅使用本机
            127.0.0.1 CDP，不修改 Codex 的安装文件。
          </p>
        </div>
        <div className={`connection-card state-${status.state}`}>
          <div className="connection-title">
            <span className="status-dot" aria-hidden="true" />
            <strong>{stateLabel[status.state]}</strong>
          </div>
          <p>{status.detail}</p>
          {status.port && <code>127.0.0.1:{status.port}</code>}
          <div className="connection-actions">
            <button className="button button-secondary" onClick={() => void refreshStatus()} disabled={isBusy}>
              刷新状态
            </button>
            <button className="button button-primary" onClick={() => void connectCodex()} disabled={isBusy}>
              {busy === "connect" ? "正在连接…" : "连接 / 启动 Codex"}
            </button>
          </div>
        </div>
      </header>

      {status.state === "runningWithoutDebugPort" && (
        <section className="warning-panel">
          <strong>当前 Codex 是未开启 CDP 的实例。</strong>
          请通过 Codex 自己的菜单或托盘正常退出，等待它完全退出后，再点击“连接 / 启动 Codex”。
          CodeSkin 不会强制结束你的 Codex 进程。
        </section>
      )}

      {error && <section className="message error-message">{error}</section>}
      {notice && <section className="message notice-message">{notice}</section>}

      <section className="section-head">
        <div>
          <p className="eyebrow">背景图库</p>
          <h2>上传的图片</h2>
          <p>原图会保存在本机；CodeSkin 同时生成适合窗口显示的 2560 × 1440 JPEG。</p>
        </div>
        <div className="section-actions">
          <input
            ref={fileInput}
            className="visually-hidden"
            type="file"
            accept="image/png,image/jpeg,image/webp"
            onChange={(event) => void uploadFile(event)}
          />
          <button
            className="button button-primary"
            disabled={isBusy}
            onClick={() => fileInput.current?.click()}
          >
            {busy === "upload" ? "正在导入…" : "上传背景图片"}
          </button>
          <button className="button button-secondary" disabled={isBusy} onClick={() => void verify()}>
            {busy === "verify" ? "正在校验…" : "校验注入"}
          </button>
          <button className="button button-danger" disabled={isBusy} onClick={() => void restore()}>
            恢复官方外观
          </button>
        </div>
      </section>

      {library === null ? (
        <section className="empty-state">正在加载本地背景图库…</section>
      ) : library.backgrounds.length === 0 ? (
        <section className="empty-state">
          <div className="empty-art" aria-hidden="true">▧</div>
          <h3>还没有背景图片</h3>
          <p>上传一张 PNG、JPEG 或 WebP 图片。它不会修改 Codex 的官方安装包。</p>
          <button className="button button-primary" disabled={isBusy} onClick={() => fileInput.current?.click()}>
            选择图片
          </button>
        </section>
      ) : (
        <section className="background-grid" aria-label="上传的 Codex 背景">
          {library.backgrounds.map((background) => {
            const active = background.id === library.selectedBackgroundId;
            const applying = busy === `apply:${background.id}`;
            return (
              <article
                className={`background-card ${active ? "background-active" : ""}`}
                key={background.id}
              >
                <button
                  className="background-select"
                  disabled={isBusy}
                  onClick={() => void apply(background)}
                  title={`应用 ${background.name}`}
                >
                  <div className="background-thumbnail">
                    {background.previewDataUrl ? (
                      <img
                        src={background.previewDataUrl}
                        alt={`${background.name} 背景预览`}
                      />
                    ) : (
                      <div className="thumbnail-fallback">图片不可用</div>
                    )}
                    <span className="thumbnail-shade" />
                    {active && <span className="active-badge">正在使用</span>}
                    {applying && <span className="active-badge">正在应用…</span>}
                  </div>
                  <div className="background-meta">
                    <strong>{background.name}</strong>
                    <span>{active ? "点击可重新应用" : "点击立即应用"}</span>
                  </div>
                </button>
                <button
                  className="background-delete"
                  aria-label={`删除 ${background.name}`}
                  disabled={isBusy}
                  onClick={() => void remove(background)}
                  title="删除这张背景图片"
                >
                  删除
                </button>
              </article>
            );
          })}
        </section>
      )}

      {verification && (
        <section className="verify-panel">
          <div className="verify-heading">
            <div>
              <p className="eyebrow">注入校验</p>
              <h2>{verification.active ? "背景层正在运行" : "未检测到完整背景层"}</h2>
            </div>
            <span className={`result-pill ${verification.active ? "result-ok" : "result-warn"}`}>
              {verification.active ? "已生效" : "需要检查"}
            </span>
          </div>
          {verification.targets.length ? (
            <div className="target-list">
              {verification.targets.map((target) => (
                <div className="target-row" key={target.targetId}>
                  <span className={`target-dot ${target.active ? "target-ok" : ""}`} />
                  <div>
                    <strong>{target.active ? "背景图、样式层均已生效" : "此页面未完整生效"}</strong>
                    <p>{target.detail}</p>
                    <small>{target.targetUrl}</small>
                  </div>
                  <span className="target-mode">{target.mode ?? "未知页面"}</span>
                </div>
              ))}
            </div>
          ) : (
            <p className="verify-empty">暂无可校验的 Codex 渲染页面。</p>
          )}
        </section>
      )}

      <footer>
        CodeSkin 是非官方视觉定制工具。它仅在运行时向本机 Codex 渲染进程注入背景层；退出或恢复后不会改动 Codex 官方文件。
      </footer>
    </main>
  );
}

export default App;
