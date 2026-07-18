(theme) => {
  const styleId = "codeskin-runtime-style";
  const wallpaperId = "codeskin-wallpaper-layer";
  const runtimeKey = "__codeskinRuntime";
  const observerKey = "__codeskinModeObserver";
  const runtimeOwner = "codeskin-runtime-v1";
  const runtimeVersion = 1;
  const ownerAttribute = "data-codeskin-theme-id";
  const modeAttribute = "data-codeskin-mode";
  const ownedAttribute = "data-codeskin-owned";
  const layerAttribute = "data-codeskin-layer";
  const runtimeAttribute = "data-codeskin-runtime";
  const FALLBACK_COLORS = Object.freeze({
    accent: "#7AA2F7",
    background: "#1A1B26",
    surface: "#24283B",
    foreground: "#C0CAF5",
    muted: "#565F89"
  });
  const WINDOWS_ABSOLUTE_PATH = /^[A-Za-z]:[\\/]/;

  const isOwnedStyle = (node) => Boolean(
    node
      && node.nodeType === 1
      && node.tagName === "STYLE"
      && node.id === styleId
      && node.getAttribute(ownedAttribute) === "true"
      && node.getAttribute(layerAttribute) === "style"
      && node.getAttribute(runtimeAttribute) === runtimeOwner
  );

  const isOwnedWallpaper = (node) => Boolean(
    node
      && node.nodeType === 1
      && node.tagName === "DIV"
      && node.id === wallpaperId
      && node.getAttribute(ownedAttribute) === "true"
      && node.getAttribute(layerAttribute) === "wallpaper"
      && node.getAttribute(runtimeAttribute) === runtimeOwner
      && node.getAttribute("aria-hidden") === "true"
  );

  const isOwnedRuntime = (runtime, root) => Boolean(
    runtime
      && typeof runtime === "object"
      && runtime.owner === runtimeOwner
      && runtime.version === runtimeVersion
      && runtime.root === root
      && (runtime.style === null || isOwnedStyle(runtime.style))
      && (runtime.wallpaper === null || isOwnedWallpaper(runtime.wallpaper))
      && (runtime.observer === null || typeof runtime.observer.disconnect === "function")
      && (runtime.pendingInstall === null || typeof runtime.pendingInstall === "function")
      && typeof runtime.modeUpdateQueued === "boolean"
  );

  const hasRuntimeProperty = () => runtimeKey in window;
  const observerGlobalIsCompatible = (runtime) => (
    !(observerKey in window) || Boolean(runtime && window[observerKey] === runtime.observer)
  );
  const currentRuntime = (root) => {
    const runtime = window[runtimeKey];
    return isOwnedRuntime(runtime, root) ? runtime : null;
  };

  const createRuntime = (root) => ({
    owner: runtimeOwner,
    version: runtimeVersion,
    root,
    style: null,
    wallpaper: null,
    observer: null,
    pendingInstall: null,
    modeUpdateQueued: false
  });

  const cancelPendingInstall = (runtime) => {
    if (typeof runtime.pendingInstall === "function") {
      document.removeEventListener("DOMContentLoaded", runtime.pendingInstall);
    }
    runtime.pendingInstall = null;
  };

  const safeColor = (value, fallback) => (
    typeof value === "string" && /^#[0-9A-Fa-f]{6}$/.test(value) ? value : fallback
  );

  const numberValue = (value, fallback) => (
    Number.isFinite(value) ? Math.min(1, Math.max(0, value)) : fallback
  );

  const windowsPathToFileUrl = (value) => {
    const parts = value.replace(/\\/g, "/").split("/");
    return `file:///${parts.map((part, index) => (
      index === 0 ? part : encodeURIComponent(part)
    )).join("/")}`;
  };

  const safeBackgroundImage = (value) => {
    if (typeof value !== "string" || value.length === 0) return "none";

    if (WINDOWS_ABSOLUTE_PATH.test(value)) {
      return `url(${JSON.stringify(windowsPathToFileUrl(value))})`;
    }

    try {
      const parsed = new URL(value);
      if (
        parsed.protocol !== "file:"
        || parsed.hostname !== ""
        || !parsed.href.startsWith("file:///")
        || parsed.pathname.startsWith("//")
        || parsed.search
        || parsed.hash
      ) {
        return "none";
      }
      return `url(${JSON.stringify(parsed.href)})`;
    } catch (_) {
      return "none";
    }
  };

  const staticIdsAreAvailable = (runtime) => {
    const existingStyle = document.getElementById(styleId);
    const existingWallpaper = document.getElementById(wallpaperId);
    return (!existingStyle || existingStyle === runtime.style)
      && (!existingWallpaper || existingWallpaper === runtime.wallpaper);
  };

  const ensureRuntimeNodes = (runtime, root) => {
    if (!staticIdsAreAvailable(runtime)) return null;

    if (!runtime.style) {
      const style = document.createElement("style");
      style.id = styleId;
      style.setAttribute(ownedAttribute, "true");
      style.setAttribute(layerAttribute, "style");
      style.setAttribute(runtimeAttribute, runtimeOwner);
      (document.head || root).appendChild(style);
      runtime.style = style;
    } else if (!runtime.style.isConnected) {
      (document.head || root).appendChild(runtime.style);
    }

    if (!runtime.wallpaper) {
      const wallpaper = document.createElement("div");
      wallpaper.id = wallpaperId;
      wallpaper.setAttribute(ownedAttribute, "true");
      wallpaper.setAttribute(layerAttribute, "wallpaper");
      wallpaper.setAttribute(runtimeAttribute, runtimeOwner);
      wallpaper.setAttribute("aria-hidden", "true");
      runtime.wallpaper = wallpaper;
    }

    if (document.body.firstChild !== runtime.wallpaper) {
      document.body.insertBefore(runtime.wallpaper, document.body.firstChild);
    }

    return { style: runtime.style, wallpaper: runtime.wallpaper };
  };

  const computeMode = () => {
    const hasMain = Boolean(document.querySelector("main, [role='main']"));
    const hasComposer = Boolean(document.querySelector("textarea, [contenteditable='true'], [role='textbox']"));
    const hasTranscript = Boolean(document.querySelector("[role='log'], [data-message-author-role]"));
    const hasCode = Boolean(document.querySelector("pre code, [data-language-for-alternating-lines]"));
    const hasProjectSurface = Boolean(document.querySelector("[role='tree'], [aria-label*='Project'], [aria-label*='项目']"));
    const hasWelcomeSurface = Boolean(document.querySelector("[data-testid*='welcome'], [aria-label*='Welcome'], [aria-label*='欢迎']"));
    const hasWorkingSurface = hasTranscript || hasCode || hasProjectSurface || (hasComposer && !hasWelcomeSurface);

    return hasMain && hasWelcomeSurface && !hasWorkingSurface ? "ambient" : "focus";
  };

  const updateMode = (root) => {
    const mode = computeMode();
    if (root.getAttribute(modeAttribute) !== mode) {
      root.setAttribute(modeAttribute, mode);
    }
    return mode;
  };

  const installModeObserver = (runtime, root) => {
    if (!observerGlobalIsCompatible(runtime)) return false;

    if (runtime.observer) runtime.observer.disconnect();

    const observer = new MutationObserver(() => {
      if (runtime.modeUpdateQueued) return;
      runtime.modeUpdateQueued = true;
      queueMicrotask(() => {
        runtime.modeUpdateQueued = false;
        if (window[runtimeKey] === runtime && isOwnedRuntime(runtime, root)) {
          updateMode(root);
        }
      });
    });
    observer.observe(document.body, {
      childList: true,
      subtree: true,
      attributes: true,
      attributeFilter: ["aria-label", "contenteditable", "data-language-for-alternating-lines", "data-message-author-role", "role", "data-testid"]
    });
    runtime.observer = observer;
    window[observerKey] = observer;
    return true;
  };

  const applyTheme = (runtime) => {
    const root = document.documentElement;
    if (
      !root
      || !document.body
      || !isOwnedRuntime(runtime, root)
      || !observerGlobalIsCompatible(runtime)
    ) return null;

    const nodes = ensureRuntimeNodes(runtime, root);
    if (!nodes) return null;

    const colors = theme && typeof theme.colors === "object" ? theme.colors : {};
    const layers = theme && typeof theme.layers === "object" ? theme.layers : {};
    const accent = safeColor(colors.accent, FALLBACK_COLORS.accent);
    const background = safeColor(colors.background, FALLBACK_COLORS.background);
    const surface = safeColor(colors.surface, FALLBACK_COLORS.surface);
    const foreground = safeColor(colors.foreground, FALLBACK_COLORS.foreground);
    const muted = safeColor(colors.muted, FALLBACK_COLORS.muted);
    const ambientOpacity = numberValue(layers.ambientOverlayOpacity, 0.20);
    const focusOpacity = numberValue(layers.focusOverlayOpacity, 0.78);
    const sidebarOpacity = numberValue(layers.sidebarOpacity, 0.58);
    const cardOpacity = numberValue(layers.cardOpacity, 0.46);

    nodes.style.textContent = `
:root[${ownerAttribute}] {
  --codeskin-accent: ${accent};
  --codeskin-background: ${background};
  --codeskin-surface: ${surface};
  --codeskin-foreground: ${foreground};
  --codeskin-muted: ${muted};
  --codeskin-ambient-overlay-opacity: ${ambientOpacity};
  --codeskin-focus-overlay-opacity: ${focusOpacity};
  --codeskin-sidebar-opacity: ${sidebarOpacity};
  --codeskin-card-opacity: ${cardOpacity};
}
:root[${ownerAttribute}][${modeAttribute}="ambient"] {
  --codeskin-current-overlay-opacity: var(--codeskin-ambient-overlay-opacity);
}
:root[${ownerAttribute}][${modeAttribute}="focus"] {
  --codeskin-current-overlay-opacity: var(--codeskin-focus-overlay-opacity);
}
#${wallpaperId}[${ownedAttribute}="true"][${layerAttribute}="wallpaper"][${runtimeAttribute}="${runtimeOwner}"] {
  position: fixed;
  inset: 0;
  pointer-events: none;
  z-index: -2;
  background-position: center;
  background-size: cover;
  background-repeat: no-repeat;
}
:root[${ownerAttribute}] main,
:root[${ownerAttribute}] [role="main"],
:root[${ownerAttribute}] [role="dialog"],
:root[${ownerAttribute}] [role="listbox"],
:root[${ownerAttribute}] [role="menu"],
:root[${ownerAttribute}] button,
:root[${ownerAttribute}] input,
:root[${ownerAttribute}] textarea,
:root[${ownerAttribute}] [contenteditable="true"],
:root[${ownerAttribute}] pre,
:root[${ownerAttribute}] [role="button"] {
  color: var(--codeskin-foreground) !important;
  background-color: color-mix(in srgb, var(--codeskin-surface) calc(var(--codeskin-current-overlay-opacity) * 100%), transparent) !important;
  border-color: color-mix(in srgb, var(--codeskin-muted) 38%, transparent) !important;
  backdrop-filter: blur(16px) saturate(118%);
}
:root[${ownerAttribute}] nav,
:root[${ownerAttribute}] aside,
:root[${ownerAttribute}] [role="navigation"] {
  color: var(--codeskin-foreground) !important;
  background-color: color-mix(in srgb, var(--codeskin-surface) calc(var(--codeskin-sidebar-opacity) * 100%), transparent) !important;
  border-color: color-mix(in srgb, var(--codeskin-muted) 42%, transparent) !important;
  backdrop-filter: blur(18px) saturate(118%);
}
:root[${ownerAttribute}] button,
:root[${ownerAttribute}] input,
:root[${ownerAttribute}] textarea,
:root[${ownerAttribute}] [contenteditable="true"],
:root[${ownerAttribute}] [role="button"] {
  accent-color: var(--codeskin-accent) !important;
}
`;

    nodes.wallpaper.style.backgroundImage = safeBackgroundImage(theme && theme.backgroundImage);
    root.setAttribute(ownerAttribute, typeof (theme && theme.id) === "string" ? theme.id : "");
    const mode = updateMode(root);
    if (!installModeObserver(runtime, root)) return null;
    return { root, style: nodes.style, wallpaper: nodes.wallpaper, mode };
  };

  const root = document.documentElement;
  let runtime = currentRuntime(root);
  if (!runtime && hasRuntimeProperty()) {
    return { active: false, pending: false, reason: "runtime-conflict" };
  }
  if (!observerGlobalIsCompatible(runtime)) {
    return { active: false, pending: false, reason: "observer-conflict" };
  }

  if (!root || !document.body) {
    if (!runtime) {
      if (document.getElementById(styleId) || document.getElementById(wallpaperId)) {
        return { active: false, pending: false, reason: "id-conflict" };
      }
      runtime = createRuntime(root);
      window[runtimeKey] = runtime;
    }

    cancelPendingInstall(runtime);
    const pendingInstall = () => {
      if (runtime.pendingInstall !== pendingInstall || window[runtimeKey] !== runtime) return;
      runtime.pendingInstall = null;
      if (runtime.root === null) runtime.root = document.documentElement;
      applyTheme(runtime);
    };
    runtime.pendingInstall = pendingInstall;
    document.addEventListener("DOMContentLoaded", pendingInstall, { once: true });
    return { active: false, pending: true };
  }

  if (!runtime) {
    if (document.getElementById(styleId) || document.getElementById(wallpaperId)) {
      return { active: false, pending: false, reason: "id-conflict" };
    }
    runtime = createRuntime(root);
    window[runtimeKey] = runtime;
  }

  cancelPendingInstall(runtime);
  const result = applyTheme(runtime);
  if (!result) return { active: false, pending: false, reason: "id-conflict" };

  return {
    active: true,
    themeId: result.root.getAttribute(ownerAttribute),
    accent: getComputedStyle(result.root).getPropertyValue("--codeskin-accent").trim(),
    wallpaperLayer: true,
    styleLayer: true,
    mode: result.mode
  };
}