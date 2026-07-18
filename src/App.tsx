import { useEffect, useMemo, useState } from "react";
import {
  applyTheme,
  connectOrStartCodex,
  importWallpaperTheme,
  inspectCodexStatus,
  loadThemeLibrary,
  renameTheme,
  restoreTheme,
  verifyTheme,
} from "./api";
import type {
  CodexConnectionState,
  CodexStatus,
  CommandError,
  Theme,
  ThemeLibrary,
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
  debugPortDetected: "本地 CDP 已发现",
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

function normalizeLibrary(library: ThemeLibrary): ThemeLibrary {
  const hasSelectedTheme = library.themes.some(
    (theme) => theme.id === library.selectedThemeId,
  );
  return hasSelectedTheme
    ? library
    : { ...library, selectedThemeId: library.themes[0]?.id ?? null };
}

function percent(value: number): string {
  return `${Math.round(value * 100)}%`;
}

function ThemePreview({ theme }: { theme: Theme }) {
  if (theme.source === "wallpaper" && theme.backgroundImage) {
    return (
      <div className="theme-preview wallpaper-preview">
        <img src={theme.backgroundImage} alt={`${theme.name} 壁纸缩略图`} />
        <span className="source-badge">WALLPAPER</span>
      </div>
    );
  }

  return (
    <div
      className="theme-preview builtin-preview"
      style={{
        background: theme.colors.background,
        color: theme.colors.foreground,
        borderColor: theme.colors.accent,
      }}
    >
      <span className="preview-rail" style={{ background: theme.colors.surface }} />
      <span className="preview-line line-one" style={{ background: theme.colors.accent }} />
      <span className="preview-line line-two" style={{ background: theme.colors.muted }} />
      <span className="preview-chip" style={{ background: theme.colors.accent }} />
    </div>
  );
}

function ColorSwatches({ theme }: { theme: Theme }) {
  const swatches = [
    ["背景", theme.colors.background],
    ["表面", theme.colors.surface],
    ["强调", theme.colors.accent],
    ["文字", theme.colors.foreground],
  ] as const;

  return (
    <div className="color-swatches" aria-label={`${theme.name} 配色`}>
      {swatches.map(([label, color]) => (
        <span
          className="color-swatch"
          key={label}
          style={{ backgroundColor: color }}
          title={`${label}: ${color}`}
        />
      ))}
    </div>
  );
}

function App() {
  const [library, setLibrary] = useState<ThemeLibrary | null>(null);
  const [status, setStatus] = useState<CodexStatus>(initialStatus);
  const [verification, setVerification] = useState<VerifyResult | null>(null);
  const [themeName, setThemeName] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const [notice, setNotice] = useState("");
  const [error, setError] = useState("");

  const selectedTheme = useMemo(
    () =>
      library?.themes.find((theme) => theme.id === library.selectedThemeId) ??
      null,
    [library],
  );
  const isBusy = busy !== null;

  useEffect(() => {
    setThemeName(selectedTheme?.source === "wallpaper" ? selectedTheme.name : "");
  }, [selectedTheme]);

  useEffect(() => {
    void initialize();
  }, []);

  async function initialize() {
    setBusy("initialize");
    setError("");
    try {
      const [nextLibrary, codexStatus] = await Promise.all([
        loadThemeLibrary(),
        inspectCodexStatus(),
      ]);
      setLibrary(normalizeLibrary(nextLibrary));
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

  function selectTheme(themeId: string) {
    setLibrary((current) =>
      current ? { ...current, selectedThemeId: themeId } : current,
    );
    setVerification(null);
    setNotice("");
  }

  async function connect() {
    setBusy("connect");
    setError("");
    setNotice("");
    try {
      const nextStatus = await connectOrStartCodex();
      setStatus(nextStatus);
      setNotice(nextStatus.detail);
    } catch (cause) {
      setError(errorMessage(cause));
      await refreshStatus();
    } finally {
      setBusy(null);
    }
  }

  async function applySelectedTheme() {
    if (!selectedTheme) return;
    setBusy("apply");
    setError("");
    setNotice("");
    try {
      const result = await applyTheme(selectedTheme.id);
      setVerification(result);
      setStatus(await inspectCodexStatus());
      setNotice(
        result.active
          ? `“${selectedTheme.name}”已注入到 ${result.targets.length} 个页面 target。`
          : "注入请求已完成，但校验未通过；请查看 target 详情。",
      );
    } catch (cause) {
      setError(errorMessage(cause));
      await refreshStatus();
    } finally {
      setBusy(null);
    }
  }

  async function verify() {
    setBusy("verify");
    setError("");
    try {
      const result = await verifyTheme();
      setVerification(result);
      setNotice(
        result.active
          ? `校验通过：${result.targets.length} 个已登记页面仍保留 CodeSkin 标记。`
          : "校验未通过或目前没有已登记的页面 target。",
      );
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(null);
    }
  }

  async function restore() {
    setBusy("restore");
    setError("");
    try {
      const result = await restoreTheme();
      setVerification(result);
      setNotice("已请求移除当前页面和后续新文档中的 CodeSkin 注入。");
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(null);
    }
  }

  async function importTheme(event: React.ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file) return;

    setBusy("import");
    setError("");
    setNotice("");
    try {
      const bytes = await file.arrayBuffer();
      const nextLibrary = await importWallpaperTheme(
        Array.from(new Uint8Array(bytes)),
        file.name,
      );
      setLibrary(normalizeLibrary(nextLibrary));
      setNotice(`已从“${file.name}”生成并选择新的壁纸主题。`);
    } catch (cause) {
      setError(`壁纸主题导入失败：${errorMessage(cause)}`);
    } finally {
      setBusy(null);
    }
  }

  async function saveThemeName() {
    if (!selectedTheme || selectedTheme.source !== "wallpaper") return;
    setBusy("rename");
    setError("");
    setNotice("");
    try {
      const nextLibrary = await renameTheme(selectedTheme.id, themeName);
      setLibrary(normalizeLibrary(nextLibrary));
      setNotice("壁纸主题名称已保存。");
    } catch (cause) {
      setError(`保存主题名称失败：${errorMessage(cause)}`);
    } finally {
      setBusy(null);
    }
  }

  return (
    <main className="app-shell">
      <header className="hero">
        <div>
          <p className="eyebrow">RUNTIME THEME LAYER</p>
          <h1>CodeSkin</h1>
          <p className="subtle">
            通过严格限定在 <code>127.0.0.1</code> 的本地 CDP，对运行中的 Codex
            渲染页施加临时视觉层；不会修改 Codex 安装文件、<code>app.asar</code> 或签名。
          </p>
        </div>
        <div className={`connection-badge state-${status.state}`}>
          <span className="status-dot" aria-hidden="true" />
          <div>
            <strong>{stateLabel[status.state]}</strong>
            <span>{status.port ? `127.0.0.1:${status.port}` : "尚无调试端口"}</span>
          </div>
        </div>
      </header>

      <section className="panel connection-panel" aria-labelledby="connection-title">
        <div className="section-heading">
          <div>
            <p className="eyebrow">CODEX CONNECTION</p>
            <h2 id="connection-title">连接与启动</h2>
          </div>
          <div className="button-row">
            <button className="button secondary" type="button" onClick={() => void refreshStatus()} disabled={isBusy}>
              刷新状态
            </button>
            <button className="button primary" type="button" onClick={() => void connect()} disabled={isBusy}>
              {busy === "connect" ? "处理中…" : "连接或启动 Codex"}
            </button>
            <button className="button ghost" type="button" onClick={() => void verify()} disabled={isBusy}>
              {busy === "verify" ? "校验中…" : "校验注入"}
            </button>
          </div>
        </div>
        <p className="status-detail">{status.detail}</p>
        {status.state === "runningWithoutDebugPort" ? (
          <p className="warning-text">当前 Codex 已运行但未开放调试端口。请先退出它，再由 CodeSkin 启动；不会强制关闭现有进程。</p>
        ) : null}
      </section>

      <section className="panel" aria-labelledby="themes-title">
        <div className="section-heading">
          <div>
            <p className="eyebrow">THEME LIBRARY {library ? `V${library.version}` : ""}</p>
            <h2 id="themes-title">主题库</h2>
            <p className="subtle compact">选择主题后应用；壁纸仅保存在 CodeSkin 本地数据目录，不会上传。</p>
          </div>
          <div className="button-row">
            <label className={`button secondary file-button ${isBusy ? "disabled" : ""}`}>
              {busy === "import" ? "生成中…" : "导入壁纸并生成主题"}
              <input type="file" accept="image/png,image/jpeg,image/webp" onChange={(event) => void importTheme(event)} disabled={isBusy} />
            </label>
            <button className="button primary" type="button" onClick={() => void applySelectedTheme()} disabled={!selectedTheme || isBusy}>
              {busy === "apply" ? "应用中…" : "应用所选主题"}
            </button>
          </div>
        </div>

        {library?.themes.length ? (
          <div className="theme-grid" aria-label="主题列表">
            {library.themes.map((theme) => (
              <button
                className={`theme-card ${selectedTheme?.id === theme.id ? "selected" : ""}`}
                key={theme.id}
                onClick={() => selectTheme(theme.id)}
                type="button"
                aria-pressed={selectedTheme?.id === theme.id}
                disabled={isBusy}
              >
                <ThemePreview theme={theme} />
                <div className="theme-card-copy">
                  <div className="theme-card-title">
                    <strong>{theme.name}</strong>
                    {theme.source === "wallpaper" ? <span className="source-label">壁纸</span> : null}
                  </div>
                  <span>{theme.description}</span>
                  <ColorSwatches theme={theme} />
                </div>
              </button>
            ))}
          </div>
        ) : (
          <p className="subtle empty-state">主题库目前没有可用主题。</p>
        )}

        {selectedTheme ? (
          <div className="selected-theme-details" aria-live="polite">
            <div>
              <p className="eyebrow">SELECTED THEME</p>
              <h3>{selectedTheme.name}</h3>
              <p className="subtle compact">{selectedTheme.description}</p>
            </div>
            <dl className="layer-list">
              <div><dt>Ambient overlay</dt><dd>{percent(selectedTheme.layers.ambientOverlayOpacity)}</dd></div>
              <div><dt>Focus overlay</dt><dd>{percent(selectedTheme.layers.focusOverlayOpacity)}</dd></div>
            </dl>
          </div>
        ) : null}

        {selectedTheme?.source === "wallpaper" ? (
          <form className="rename-form" onSubmit={(event) => { event.preventDefault(); void saveThemeName(); }}>
            <label htmlFor="wallpaper-theme-name">壁纸主题名称</label>
            <div className="rename-controls">
              <input
                id="wallpaper-theme-name"
                type="text"
                value={themeName}
                onChange={(event) => setThemeName(event.target.value)}
                disabled={isBusy}
                required
              />
              <button className="button secondary" type="submit" disabled={isBusy || !themeName.trim()}>
                {busy === "rename" ? "保存中…" : "保存名称"}
              </button>
            </div>
          </form>
        ) : null}
      </section>

      <section className="panel restore-panel" aria-label="恢复工具">
        <div>
          <p className="eyebrow">SAFE EXIT</p>
          <h2>恢复原始外观</h2>
          <p className="subtle compact">移除 CodeSkin 自己创建的 style 标记、主题属性和刷新时注入的脚本；不触碰 Codex 原有样式。</p>
        </div>
        <button className="button danger" type="button" onClick={() => void restore()} disabled={isBusy}>
          {busy === "restore" ? "恢复中…" : "恢复 Codex 原始外观"}
        </button>
      </section>

      {notice ? <p className="feedback success" role="status">{notice}</p> : null}
      {error ? <p className="feedback error" role="alert">{error}</p> : null}

      {verification ? (
        <section className="panel verification" aria-labelledby="verification-title">
          <div className="section-heading">
            <div>
              <p className="eyebrow">VERIFY RESULT</p>
              <h2 id="verification-title">{verification.active ? "注入校验通过" : "注入校验未通过"}</h2>
            </div>
            <span className={`verification-chip ${verification.active ? "active" : "inactive"}`}>
              {verification.active ? "ACTIVE" : "INACTIVE"}
            </span>
          </div>
          {verification.targets.length === 0 ? (
            <p className="subtle compact">当前没有可校验的、由 CodeSkin 登记的页面 target。</p>
          ) : (
            <ul className="target-list">
              {verification.targets.map((target) => (
                <li key={target.targetId}>
                  <span className={`target-dot ${target.active ? "active" : "inactive"}`} aria-hidden="true" />
                  <div>
                    <strong>{target.active ? "已检测到主题标记" : "未检测到主题标记"}</strong>
                    <code>{target.targetUrl || "about:blank"}</code>
                    <div className="target-layers" aria-label="页面图层状态">
                      <span className={target.wallpaperLayer ? "active" : "inactive"}>wallpaper {target.wallpaperLayer ? "on" : "off"}</span>
                      <span className={target.styleLayer ? "active" : "inactive"}>style {target.styleLayer ? "on" : "off"}</span>
                      <span className="mode-chip">mode {target.mode ?? "unknown"}</span>
                    </div>
                    <p>{target.detail}</p>
                  </div>
                </li>
              ))}
            </ul>
          )}
        </section>
      ) : null}

      <section className="notice" role="note">
        <strong>非官方工具。</strong> CodeSkin 不隶属于、也未获 OpenAI 或 Codex 官方支持。它只做运行时视觉层修改；Codex 更新可能改变 DOM，届时请先使用“校验注入”，必要时恢复原始外观。
      </section>
    </main>
  );
}

export default App;

