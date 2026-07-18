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
  // Rust creates this only from a CodeSkin-managed derived JPEG. Do not allow
  // file, http(s), SVG, or arbitrary data URLs in the renderer context.
  const JPEG_DATA_URL = /^data:image\/jpeg;base64,[A-Za-z0-9+/]+={0,2}$/;
  const MAX_JPEG_DATA_URL_LENGTH = 24 * 1024 * 1024;

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

  const colorSchemeFor = (hex) => {
    const channels = [1, 3, 5].map((index) => Number.parseInt(hex.slice(index, index + 2), 16) / 255);
    const linear = channels.map((value) => (
      value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4
    ));
    const luminance = 0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2];
    return luminance < 0.46 ? "dark" : "light";
  };

  const safeBackgroundImage = (value) => {
    if (
      typeof value !== "string"
      || value.length === 0
      || value.length > MAX_JPEG_DATA_URL_LENGTH
      || !JPEG_DATA_URL.test(value)
    ) {
      return "none";
    }
    return `url(${JSON.stringify(value)})`;
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
      // Portal menus and the environment summary can be created after initial injection.
      // The stylesheet below is global, so it matches those nodes on arrival; observing
      // their state/class changes also keeps focus/ambient mode detection current.
      attributeFilter: [
        "aria-label", "aria-haspopup", "aria-expanded", "contenteditable", "class",
        "data-language-for-alternating-lines", "data-message-author-role", "data-state", "role", "data-testid"
      ]
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
    const contrast = theme && typeof theme.contrast === "object" && theme.contrast !== null
      ? theme.contrast
      : {};
    const accent = safeColor(colors.accent, FALLBACK_COLORS.accent);
    const secondary = safeColor(colors.secondary, accent);
    const background = safeColor(colors.background, FALLBACK_COLORS.background);
    const surface = safeColor(colors.surface, FALLBACK_COLORS.surface);
    const foreground = safeColor(colors.foreground, FALLBACK_COLORS.foreground);
    const muted = safeColor(colors.muted, FALLBACK_COLORS.muted);
    // Saved themes from earlier versions can contain high opacity values. Cap them
    // here as a safety boundary so an update never recreates a full-window veil.
    const ambientOpacity = Math.min(numberValue(layers.ambientOverlayOpacity, 0.12), 0.14);
    const focusOpacity = Math.min(numberValue(layers.focusOverlayOpacity, 0.18), 0.22);
    const cardOpacity = Math.min(numberValue(layers.cardOpacity, 0.18), 0.22);
    const safePanelOpacity = (value, fallback) => Math.min(
      0.45,
      Math.max(0.16, numberValue(value, fallback))
    );
    const safeBlur = (value, fallback) => Math.round(Math.min(
      16,
      Math.max(8, Number.isFinite(value) ? value : fallback)
    ));
    const safeTextShadow = (value, fallback) => (
      value === "0 1px 2px rgba(0,0,0,0.4)"
      || value === "0 1px 3px rgba(0,0,0,0.6)"
      || value === "0 1px 2px rgba(255,255,255,0.4)"
      || value === "0 1px 3px rgba(255,255,255,0.6)"
    ) ? value : fallback;
    // Sidebar, reading content, and environment text stay directly on the wallpaper.
    // The saved contrast sample chooses their foreground and subtle shadow on every image apply.
    const textContrastRegion = (value) => {
      const region = value && typeof value === "object" ? value : {};
      const regionForeground = safeColor(region.foreground, foreground);
      const isLightText = colorSchemeFor(regionForeground) === "light";
      return {
        foreground: regionForeground,
        muted: safeColor(region.muted, muted),
        textShadow: safeTextShadow(
          region.textShadow,
          isLightText ? "0 1px 3px rgba(0,0,0,0.6)" : "0 1px 3px rgba(255,255,255,0.6)"
        )
      };
    };
    const composerContrastRegion = (value) => {
      const region = value && typeof value === "object" ? value : {};
      const text = textContrastRegion(region);
      return {
        ...text,
        panelColor: safeColor(region.panelColor, surface),
        panelOpacity: safePanelOpacity(region.panelOpacity, Math.max(0.24, cardOpacity)),
        blurPx: safeBlur(region.blurPx, 12)
      };
    };
    const sidebar = textContrastRegion(contrast.sidebar);
    const content = textContrastRegion(contrast.content);
    // Themes saved before the header region existed are intentionally accepted.
    // Re-applying any saved wallpaper re-analyses it in Rust; this is only a
    // compatibility fallback while an old payload is still active.
    const header = textContrastRegion(contrast.header || contrast.content);
    const infoPanel = textContrastRegion(contrast.infoPanel);
    const composer = composerContrastRegion(contrast.composer);
    const colorScheme = colorSchemeFor(surface);

    nodes.style.textContent = `
:root[${ownerAttribute}] {
  color-scheme: ${colorScheme};
  --codeskin-accent: ${accent};
  --codeskin-secondary: ${secondary};
  --codeskin-background: ${background};
  --codeskin-surface: ${surface};
  --codeskin-foreground: ${foreground};
  --codeskin-muted: ${muted};
  --codeskin-ambient-overlay-opacity: ${ambientOpacity};
  --codeskin-focus-overlay-opacity: ${focusOpacity};
  --codeskin-content-foreground: ${content.foreground};
  --codeskin-content-muted: ${content.muted};
  --codeskin-content-text-shadow: ${content.textShadow};
  --codeskin-header-foreground: ${header.foreground};
  --codeskin-header-muted-foreground: ${header.muted};
  --codeskin-header-icon-foreground: ${header.foreground};
  --codeskin-header-text-shadow: ${header.textShadow};
  --codeskin-sidebar-foreground: ${sidebar.foreground};
  --codeskin-sidebar-muted: ${sidebar.muted};
  --codeskin-sidebar-text-shadow: ${sidebar.textShadow};
  --codeskin-info-foreground: ${infoPanel.foreground};
  --codeskin-info-muted: ${infoPanel.muted};
  --codeskin-info-text-shadow: ${infoPanel.textShadow};
  --codeskin-composer-foreground: ${composer.foreground};
  --codeskin-composer-muted: ${composer.muted};
  --codeskin-composer-panel-color: ${composer.panelColor};
  --codeskin-composer-panel-opacity: ${composer.panelOpacity};
  --codeskin-composer-blur: ${composer.blurPx}px;
  --codeskin-composer-text-shadow: ${composer.textShadow};
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
  z-index: 0;
  background-position: center;
  background-size: cover;
  background-repeat: no-repeat;
}
:root[${ownerAttribute}],
:root[${ownerAttribute}] body {
  background: transparent !important;
}
:root[${ownerAttribute}] #root {
  position: relative !important;
  z-index: 1;
  background: transparent !important;
}
/* The page itself stays sharp and transparent: no full-page veil and no blur. */
:root[${ownerAttribute}] .main-surface {
  color: var(--codeskin-content-foreground) !important;
  background: transparent !important;
  background-image: none !important;
  backdrop-filter: none !important;
  -webkit-backdrop-filter: none !important;
}
:root[${ownerAttribute}] .main-surface [class*="bg-gradient-to-"][class*="from-token-main-surface-primary"] {
  background-image: none !important;
}
/* Text floats directly on the wallpaper. Contrast is regional: no non-composer panel,
   blur, border, background, or box shadow is introduced by CodeSkin. The header
   is sampled from the actual full-width top strip rather than the content area. */
:root[${ownerAttribute}] .app-header-tint {
  color: var(--codeskin-header-foreground) !important;
  background: transparent !important;
  text-shadow: var(--codeskin-header-text-shadow) !important;
}
/* Current Codex top menu triggers live in .app-header-tint.application-menu-top-bar.
   Scope this selector so an identically-tokened button in the main surface is not
   accidentally recoloured as header UI. Do not require the tertiary-token class:
   the active View trigger replaces it with menubar-selection classes. !important is
   required because Codex utility classes set the token text colour directly. */
:root[${ownerAttribute}] .app-header-tint[class*="application-menu-top-bar"]
button.no-drag[aria-haspopup="menu"] {
  color: var(--codeskin-header-foreground) !important;
  text-shadow: var(--codeskin-header-text-shadow) !important;
}
/* Header-local navigation and renderer controls inherit the header palette too.
   SVG icons in Codex use currentColor, so no blanket fill override is needed. */
:root[${ownerAttribute}] .app-header-tint :is(
  button[class*="text-token-text-tertiary"], [role="button"][class*="text-token-text-tertiary"]
) {
  color: var(--codeskin-header-icon-foreground) !important;
  text-shadow: var(--codeskin-header-text-shadow) !important;
}
:root[${ownerAttribute}] .app-shell-left-panel {
  color: var(--codeskin-sidebar-foreground) !important;
  background: transparent !important;
}
:root[${ownerAttribute}] .app-shell-left-panel :is(
  [class*="text-token-text-primary"], [class*="text-token-text-secondary"], [class*="text-token-text-tertiary"],
  [class*="text-token-text"], span, p, a, h1, h2, h3, label
) {
  color: var(--codeskin-sidebar-foreground) !important;
  text-shadow: var(--codeskin-sidebar-text-shadow) !important;
}
:root[${ownerAttribute}] .main-surface :is(
  [class*="_markdownContent_"], [class*="_codeBlock_"], [class*="bg-token-text-code-block-background"],
  [data-message-author-role], [data-testid*="message"], [data-testid*="conversation-turn"]
) {
  color: var(--codeskin-content-foreground) !important;
  background: transparent !important;
}
:root[${ownerAttribute}] .main-surface :is(
  [class*="_markdownContent_"], [class*="_codeBlock_"], [class*="bg-token-text-code-block-background"],
  [data-message-author-role], [data-testid*="message"], [data-testid*="conversation-turn"]
) :is(pre, code) {
  background: transparent !important;
}
:root[${ownerAttribute}] .main-surface :is(
  [class*="_markdownContent_"], [class*="_codeBlock_"], [class*="bg-token-text-code-block-background"],
  [data-message-author-role], [data-testid*="message"], [data-testid*="conversation-turn"]
) :is(h1, h2, h3, h4, p, span, li, blockquote, code, pre, label, a) {
  color: var(--codeskin-content-foreground) !important;
  text-shadow: var(--codeskin-content-text-shadow) !important;
}
:root[${ownerAttribute}] .main-surface [class~="group/activity-header"],
:root[${ownerAttribute}] .main-surface [class~="group/activity-header"] :is(span, p, label),
:root[${ownerAttribute}] .main-surface :is(
  [class*="text-token-text-primary"], [class*="text-token-text-secondary"], [class*="text-token-text-tertiary"]
) {
  color: var(--codeskin-content-foreground) !important;
  background: transparent !important;
  text-shadow: var(--codeskin-content-text-shadow) !important;
}
:root[${ownerAttribute}] [class*="bg-token-foreground/5"],
:root[${ownerAttribute}] [class*="bg-token-foreground/5"] :is(
  [class*="text-token-text"], h1, h2, h3, p, span, label
) {
  color: var(--codeskin-info-foreground) !important;
  background: transparent !important;
  text-shadow: var(--codeskin-info-text-shadow) !important;
}
/* The floating environment summary is mounted outside .main-surface. Its own
   token classes otherwise win over inherited wallpaper-aware text colors. Keep
   the native card surface untouched; only its foreground follows the info sample. */
:root[${ownerAttribute}] [class*="bg-token-dropdown-background"]:has([class~="group/summary-panel-item"]) {
  color: var(--codeskin-info-foreground) !important;
  text-shadow: var(--codeskin-info-text-shadow) !important;
}
:root[${ownerAttribute}] [class*="bg-token-dropdown-background"]:has([class~="group/summary-panel-item"]) :is(
  [class*="text-token-text"], [class*="text-token-foreground"], [class*="text-token-description-foreground"],
  button, span, p, label, a, svg
) {
  color: var(--codeskin-info-foreground) !important;
  text-shadow: var(--codeskin-info-text-shadow) !important;
}
/* Renderer popup menu contents keep their own main-content palette. The header-only
   rule above targets the visible title/application-menu triggers, not portal content. */
:root[${ownerAttribute}] [role="menu"] {
  color: var(--codeskin-content-foreground) !important;
  text-shadow: var(--codeskin-content-text-shadow) !important;
}
:root[${ownerAttribute}] [role="menu"] :is(
  [role^="menuitem"], [class*="text-token-text"], [class*="text-token-foreground"],
  [class*="text-token-description-foreground"], span, p, label, kbd, svg
) {
  color: var(--codeskin-content-foreground) !important;
  text-shadow: var(--codeskin-content-text-shadow) !important;
}
/* The pre-existing bottom composer remains the only CodeSkin glass surface. */
:root[${ownerAttribute}] .composer-surface-chrome {
  color: var(--codeskin-composer-foreground) !important;
  background-color: color-mix(in srgb, var(--codeskin-composer-panel-color) calc(var(--codeskin-composer-panel-opacity) * 100%), transparent) !important;
  border-color: color-mix(in srgb, var(--codeskin-composer-muted) 30%, transparent) !important;
  backdrop-filter: blur(var(--codeskin-composer-blur)) saturate(112%);
  -webkit-backdrop-filter: blur(var(--codeskin-composer-blur)) saturate(112%);
  box-shadow: 0 10px 30px color-mix(in srgb, var(--codeskin-composer-panel-color) 20%, transparent) !important;
}
:root[${ownerAttribute}] .composer-surface-chrome :is(
  [class*="text-token-text"], h1, h2, h3, p, span, label, input, textarea, [contenteditable="true"]
) {
  color: var(--codeskin-composer-foreground) !important;
  text-shadow: var(--codeskin-composer-text-shadow) !important;
}
:root[${ownerAttribute}] :focus-visible {
  outline-color: color-mix(in srgb, var(--codeskin-secondary) 78%, white) !important;
  accent-color: var(--codeskin-accent);
}
`;

    nodes.wallpaper.style.backgroundImage = safeBackgroundImage(theme && theme.backgroundImage);
    root.setAttribute(ownerAttribute, typeof (theme && theme.id) === "string" ? theme.id : "");
    root.setAttribute("data-codeskin-color-scheme", colorScheme);
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