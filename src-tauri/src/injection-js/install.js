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
    const hasTranscript = Boolean(document.querySelector("[role='log'], [data-message-author-role]"))
      || Boolean(document.querySelector("[data-thread-find-target='conversation'], [data-user-message-bubble='true']"));
    const hasCode = Boolean(document.querySelector("pre code, [data-language-for-alternating-lines]"));
    // The current Codex welcome screen has neither a stable welcome test id nor
    // an aria label, but it does contain a composer. Treat transcript/code as the
    // reliable focus signal; welcome and settings remain ambient glass surfaces.
    return hasMain && !hasTranscript && !hasCode ? "ambient" : "focus";
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
    // Keep the wallpaper visually open. Region-specific foregrounds and local
    // glass surfaces carry readability; the full-window veil remains deliberately light.
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
    const safeTextShadow = (value, fallback) => {
      if (typeof value !== "string") return fallback;
      const match = value.match(
        /^0 1px (?:2|3)px rgba\((?:0,0,0|255,255,255),(0(?:\.\d+)?|1(?:\.0+)?)\)$/
      );
      if (!match) return fallback;
      const alpha = Number(match[1]);
      return alpha >= 0.4 && alpha <= 0.6 ? value : fallback;
    };
    const textContrastRegion = (value, fallbackValue) => {
      const fallbackRegion = fallbackValue && typeof fallbackValue === "object" ? fallbackValue : {};
      const region = value && typeof value === "object" ? value : fallbackRegion;
      const regionForeground = safeColor(region.foreground, safeColor(fallbackRegion.foreground, foreground));
      const isLightText = colorSchemeFor(regionForeground) === "light";
      return {
        foreground: regionForeground,
        muted: safeColor(region.muted, safeColor(fallbackRegion.muted, muted)),
        textShadow: safeTextShadow(
          region.textShadow,
          safeTextShadow(
            fallbackRegion.textShadow,
            isLightText ? "0 1px 3px rgba(0,0,0,0.6)" : "0 1px 3px rgba(255,255,255,0.6)"
          )
        )
      };
    };
    const glassContrastRegion = (value, fallbackValue) => {
      const fallbackRegion = fallbackValue && typeof fallbackValue === "object" ? fallbackValue : {};
      const region = value && typeof value === "object" ? value : fallbackRegion;
      const text = textContrastRegion(region, fallbackRegion);
      const panelOpacity = safePanelOpacity(
        region.panelOpacity,
        safePanelOpacity(fallbackRegion.panelOpacity, Math.max(0.24, cardOpacity))
      );
      return {
        ...text,
        panelColor: safeColor(region.panelColor, safeColor(fallbackRegion.panelColor, surface)),
        panelOpacity,
        hoverOpacity: Math.min(0.52, panelOpacity + 0.08),
        elevatedOpacity: Math.min(0.56, panelOpacity + 0.12),
        blurPx: safeBlur(region.blurPx, safeBlur(fallbackRegion.blurPx, 12))
      };
    };
    const content = glassContrastRegion(contrast.content);
    const sidebar = glassContrastRegion(contrast.sidebar, contrast.content);
    // Themes saved before the header region existed are intentionally accepted.
    const header = glassContrastRegion(contrast.header, contrast.content);
    const infoPanel = glassContrastRegion(contrast.infoPanel, contrast.content);
    const composer = glassContrastRegion(contrast.composer, contrast.content);
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
  --codeskin-content-panel-color: ${content.panelColor};
  --codeskin-content-panel-opacity: ${content.panelOpacity};
  --codeskin-content-hover-opacity: ${content.hoverOpacity};
  --codeskin-content-elevated-opacity: ${content.elevatedOpacity};
  --codeskin-content-blur: ${content.blurPx}px;
  --codeskin-header-foreground: ${header.foreground};
  --codeskin-header-muted-foreground: ${header.muted};
  --codeskin-header-icon-foreground: ${header.foreground};
  --codeskin-header-text-shadow: ${header.textShadow};
  --codeskin-header-panel-color: ${header.panelColor};
  --codeskin-header-panel-opacity: ${header.panelOpacity};
  --codeskin-header-hover-opacity: ${header.hoverOpacity};
  --codeskin-header-elevated-opacity: ${header.elevatedOpacity};
  --codeskin-header-blur: ${header.blurPx}px;
  --codeskin-sidebar-foreground: ${sidebar.foreground};
  --codeskin-sidebar-muted: ${sidebar.muted};
  --codeskin-sidebar-text-shadow: ${sidebar.textShadow};
  --codeskin-sidebar-panel-color: ${sidebar.panelColor};
  --codeskin-sidebar-panel-opacity: ${sidebar.panelOpacity};
  --codeskin-sidebar-hover-opacity: ${sidebar.hoverOpacity};
  --codeskin-sidebar-elevated-opacity: ${sidebar.elevatedOpacity};
  --codeskin-sidebar-blur: ${sidebar.blurPx}px;
  --codeskin-wallpaper-veil: ${background};
  --codeskin-info-foreground: ${infoPanel.foreground};
  --codeskin-info-muted: ${infoPanel.muted};
  --codeskin-info-text-shadow: ${infoPanel.textShadow};
  --codeskin-info-panel-color: ${infoPanel.panelColor};
  --codeskin-info-panel-opacity: ${infoPanel.panelOpacity};
  --codeskin-info-hover-opacity: ${infoPanel.hoverOpacity};
  --codeskin-info-elevated-opacity: ${infoPanel.elevatedOpacity};
  --codeskin-info-blur: ${infoPanel.blurPx}px;
  --codeskin-composer-foreground: ${composer.foreground};
  --codeskin-composer-muted: ${composer.muted};
  --codeskin-composer-panel-color: ${composer.panelColor};
  --codeskin-composer-panel-opacity: ${composer.panelOpacity};
  --codeskin-composer-hover-opacity: ${composer.hoverOpacity};
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
  overflow: hidden;
}
#${wallpaperId}[${ownedAttribute}="true"][${layerAttribute}="wallpaper"][${runtimeAttribute}="${runtimeOwner}"]::after {
  content: "";
  position: absolute;
  inset: 0;
  background: var(--codeskin-wallpaper-veil);
  opacity: var(--codeskin-current-overlay-opacity);
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
/* The header remains visually open. Local glass starts at the sidebar and
   elevated controls, while the transcript/code canvas stays transparent. Each
   surface uses the wallpaper sample for the region it occupies. */
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
/* The current task title is nested in a token-colour wrapper inside this stable
   header context surface. Cover only that real header control and its title leaf. */
:root[${ownerAttribute}] [data-testid="app-shell-header-context-menu-surface"] :is([class~="text-token-foreground"], [class~="text-token-foreground"] *) {
  color: var(--codeskin-header-foreground) !important;
  text-shadow: var(--codeskin-header-text-shadow) !important;
}
/* The paired "open location" control is a real header-local fog surface. Use
   the uploaded wallpaper's sampled header glass instead of Codex's opaque fog
   token. Match both halves by their structural utility tokens, never localized text. */
@layer base {
:root[${ownerAttribute}] [data-testid="app-shell-header-context-menu-surface"] div[class~="inline-flex"][class~="items-stretch"][class~="overflow-hidden"][class~="rounded-lg"] button[class~="border-token-border"][class~="text-token-button-tertiary-foreground"][class~="bg-token-bg-fog"]:is([class~="rounded-r-none"], [class~="rounded-l-none"]) {
  color: var(--codeskin-header-foreground) !important;
  background-color: color-mix(in srgb, var(--codeskin-header-panel-color) calc(var(--codeskin-header-panel-opacity) * 100%), transparent) !important;
  border-color: color-mix(in srgb, var(--codeskin-header-foreground) 22%, transparent) !important;
  backdrop-filter: blur(var(--codeskin-header-blur)) saturate(112%);
  -webkit-backdrop-filter: blur(var(--codeskin-header-blur)) saturate(112%);
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--codeskin-header-foreground) 7%, transparent) !important;
  text-shadow: var(--codeskin-header-text-shadow) !important;
}
:root[${ownerAttribute}] [data-testid="app-shell-header-context-menu-surface"] div[class~="inline-flex"][class~="items-stretch"][class~="overflow-hidden"][class~="rounded-lg"] button[class~="border-token-border"][class~="text-token-button-tertiary-foreground"][class~="bg-token-bg-fog"]:is([class~="rounded-r-none"], [class~="rounded-l-none"]):is(:hover, :focus-visible, [data-state="open"]) {
  color: var(--codeskin-header-foreground) !important;
  background-color: color-mix(in srgb, var(--codeskin-header-panel-color) calc(var(--codeskin-header-hover-opacity) * 100%), transparent) !important;
  border-color: color-mix(in srgb, var(--codeskin-header-foreground) 32%, transparent) !important;
}
:root[${ownerAttribute}] [data-testid="app-shell-header-context-menu-surface"] div[class~="inline-flex"][class~="items-stretch"][class~="overflow-hidden"][class~="rounded-lg"] button[class~="border-token-border"][class~="text-token-button-tertiary-foreground"][class~="bg-token-bg-fog"]:is([class~="rounded-r-none"], [class~="rounded-l-none"]) svg[class~="opacity-50"] {
  color: var(--codeskin-header-foreground) !important;
  opacity: 0.82 !important;
}
}
/* Header-local navigation and renderer controls inherit the header palette too.
   SVG icons in Codex use currentColor, so no blanket fill override is needed. */
:root[${ownerAttribute}] .app-header-tint :is(
  button[class*="text-token-text-tertiary"], [role="button"][class*="text-token-text-tertiary"]
) {
  color: var(--codeskin-header-icon-foreground) !important;
  text-shadow: var(--codeskin-header-text-shadow) !important;
}

/* The responsive sidebar visibility toggle keeps the foreground/5 token even
   while pressed, which resolves to Codex dark ink over the opened dark glass.
   Match its stable state and complete class tokens without locale-specific aria text. */
:root[${ownerAttribute}] .app-header-tint button[aria-pressed][class~="aspect-square"][class~="text-token-foreground"][class~="bg-token-foreground/5"] {
  color: var(--codeskin-header-icon-foreground) !important;
  background-color: color-mix(in srgb, var(--codeskin-header-foreground) 13%, transparent) !important;
  border-color: color-mix(in srgb, var(--codeskin-header-foreground) 20%, transparent) !important;
  text-shadow: var(--codeskin-header-text-shadow) !important;
}
:root[${ownerAttribute}] .app-header-tint button[aria-pressed][class~="aspect-square"][class~="text-token-foreground"][class~="bg-token-foreground/5"]:is(:hover, :focus-visible, [aria-pressed="true"]) {
  color: var(--codeskin-header-icon-foreground) !important;
  background-color: color-mix(in srgb, var(--codeskin-header-foreground) 20%, transparent) !important;
  border-color: color-mix(in srgb, var(--codeskin-header-foreground) 30%, transparent) !important;
}
:root[${ownerAttribute}] .app-shell-left-panel {
  color: var(--codeskin-sidebar-foreground) !important;
  background-color: color-mix(in srgb, var(--codeskin-sidebar-panel-color) calc(var(--codeskin-sidebar-panel-opacity) * 100%), transparent) !important;
  border-right-color: color-mix(in srgb, var(--codeskin-sidebar-foreground) 18%, transparent) !important;
  backdrop-filter: blur(var(--codeskin-sidebar-blur)) saturate(112%);
  -webkit-backdrop-filter: blur(var(--codeskin-sidebar-blur)) saturate(112%);
  box-shadow: 12px 0 34px color-mix(in srgb, var(--codeskin-sidebar-panel-color) 24%, transparent) !important;
}
:root[${ownerAttribute}] .app-shell-left-panel :is(
  [class*="text-token-text-primary"], [class*="text-token-text-secondary"], [class*="text-token-text-tertiary"],
  [class*="text-token-text"], span, p, a, h1, h2, h3, label
) {
  color: var(--codeskin-sidebar-foreground) !important;
  text-shadow: var(--codeskin-sidebar-text-shadow) !important;
}
:root[${ownerAttribute}] .app-shell-left-panel :is(
  [class*="text-token-text-secondary"], [class*="text-token-text-tertiary"],
  [class*="text-token-description-foreground"]
) {
  color: var(--codeskin-sidebar-muted) !important;
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
/* Activity summaries use Tailwind's base layer because current Codex applies
   important utilities from its utilities layer. Important cascade-layer priority is
   reversed, so an unlayered override cannot win regardless of selector specificity.
   The semantic token class covers both text and currentColor SVG icons without
   recolouring unrelated transcript icons. */
@layer base {
  :root[${ownerAttribute}] body #root .main-surface [class~="group/activity-header"] [class*="text-token-conversation-summary"] {
    color: var(--codeskin-content-muted) !important;
    text-shadow: var(--codeskin-content-text-shadow) !important;
  }
  :root[${ownerAttribute}] body #root .main-surface [class~="group/activity-header"] :is([class*="text-token-conversation-body"], [class*="text-token-conversation-body"] :is(span, p, label, svg)) {
    color: var(--codeskin-content-muted) !important;
    text-shadow: var(--codeskin-content-text-shadow) !important;
  }
  /* Current Codex renders the collapsed processed-time summary inside this
     nested token span; keep the selector on the real expandable status button. */
  :root[${ownerAttribute}] body #root .main-surface button[aria-expanded] [class~="text-token-conversation-body"] {
    color: var(--codeskin-content-muted) !important;
    text-shadow: var(--codeskin-content-text-shadow) !important;
  }
}
/* Welcome and settings use the same stable base glass. Current Codex exposes
   suggestion cards through group/home-suggestions and the home utility bar; the
   token-surface fallback also covers settings cards without touching focus chat. */
:root[${ownerAttribute}][${modeAttribute}="ambient"] .main-surface {
  color: var(--codeskin-content-foreground) !important;
}
:root[${ownerAttribute}][${modeAttribute}="ambient"] .main-surface :is(
  h1, h2, h3, h4, p, span, label, a,
  [class*="text-token-text"], [class*="text-token-foreground"]
) {
  color: var(--codeskin-content-foreground) !important;
  text-shadow: var(--codeskin-content-text-shadow) !important;
}
:root[${ownerAttribute}][${modeAttribute}="ambient"] .main-surface :is(
  [class*="text-token-text-secondary"], [class*="text-token-text-tertiary"],
  [class*="text-token-description-foreground"]
) {
  color: var(--codeskin-content-muted) !important;
}
:root[${ownerAttribute}][${modeAttribute}="ambient"] .main-surface [class~="group/home-suggestions"] button[class*="bg-token-main-surface-primary"] {
  color: var(--codeskin-content-foreground) !important;
  background-color: color-mix(in srgb, var(--codeskin-content-panel-color) calc(var(--codeskin-content-panel-opacity) * 100%), transparent) !important;
  border-color: color-mix(in srgb, var(--codeskin-content-foreground) 24%, transparent) !important;
  backdrop-filter: blur(var(--codeskin-content-blur)) saturate(112%);
  -webkit-backdrop-filter: blur(var(--codeskin-content-blur)) saturate(112%);
  box-shadow: 0 12px 32px color-mix(in srgb, black 28%, transparent) !important;
  transition: transform 160ms ease, background-color 160ms ease, border-color 160ms ease, box-shadow 160ms ease;
}
:root[${ownerAttribute}][${modeAttribute}="ambient"] .main-surface [class~="group/home-suggestions"] button[class*="bg-token-main-surface-primary"]:hover {
  transform: translateY(-1px);
  background-color: color-mix(in srgb, var(--codeskin-content-panel-color) calc(var(--codeskin-content-hover-opacity) * 100%), transparent) !important;
  border-color: color-mix(in srgb, var(--codeskin-secondary) 54%, var(--codeskin-content-foreground)) !important;
}
:root[${ownerAttribute}][${modeAttribute}="ambient"] .main-surface [class*="_homeUtilityBar_"] {
  color: var(--codeskin-content-foreground) !important;
  background-color: color-mix(in srgb, var(--codeskin-content-panel-color) calc(var(--codeskin-content-panel-opacity) * 100%), transparent) !important;
  border-color: color-mix(in srgb, var(--codeskin-content-foreground) 22%, transparent) !important;
  backdrop-filter: blur(var(--codeskin-content-blur)) saturate(112%);
  -webkit-backdrop-filter: blur(var(--codeskin-content-blur)) saturate(112%);
}
:root[${ownerAttribute}][${modeAttribute}="ambient"] .main-surface :is([class*="bg-token-main-surface-primary"], [class*="bg-token-main-surface-secondary"], [class*="bg-token-dropdown-background"]) {
  color: var(--codeskin-content-foreground) !important;
  background-color: color-mix(in srgb, var(--codeskin-content-panel-color) calc(var(--codeskin-content-panel-opacity) * 100%), transparent) !important;
  border-color: color-mix(in srgb, var(--codeskin-content-foreground) 22%, transparent) !important;
  backdrop-filter: blur(var(--codeskin-content-blur)) saturate(112%);
  -webkit-backdrop-filter: blur(var(--codeskin-content-blur)) saturate(112%);
  box-shadow: 0 12px 32px color-mix(in srgb, black 26%, transparent) !important;
}
/* Current settings sections do not expose token background classes. Their stable
   shape is an overflow-hidden rounded card with the token border. Keep this
   ambient-only so rounded transcript and code containers remain untouched. */
:root[${ownerAttribute}][${modeAttribute}="ambient"] .main-surface [class~="overflow-hidden"][class~="rounded-2xl"][class~="border-token-border"] {
  color: var(--codeskin-content-foreground) !important;
  background-color: color-mix(in srgb, var(--codeskin-content-panel-color) calc(var(--codeskin-content-panel-opacity) * 100%), transparent) !important;
  border-color: color-mix(in srgb, var(--codeskin-content-foreground) 22%, transparent) !important;
  backdrop-filter: blur(var(--codeskin-content-blur)) saturate(112%);
  -webkit-backdrop-filter: blur(var(--codeskin-content-blur)) saturate(112%);
  box-shadow: 0 12px 32px color-mix(in srgb, black 26%, transparent) !important;
}
:root[${ownerAttribute}][${modeAttribute}="ambient"] .main-surface [class~="overflow-hidden"][class~="rounded-2xl"][class~="border-token-border"] > *::after {
  background-color: color-mix(in srgb, var(--codeskin-content-foreground) 14%, transparent) !important;
}
/* Settings controls use bg-token-bg-fog, which otherwise stays nearly opaque
   white. Match the complete class token so hover:* utility fragments elsewhere
   cannot be mistaken for a surface. */
:root[${ownerAttribute}][${modeAttribute}="ambient"] .main-surface [class~="bg-token-bg-fog"] {
  color: var(--codeskin-content-foreground) !important;
  background-color: color-mix(in srgb, var(--codeskin-content-panel-color) calc(var(--codeskin-content-panel-opacity) * 100%), transparent) !important;
  border-color: color-mix(in srgb, var(--codeskin-content-foreground) 20%, transparent) !important;
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--codeskin-content-foreground) 8%, transparent) !important;
}
:root[${ownerAttribute}][${modeAttribute}="ambient"] .main-surface [class~="bg-token-bg-fog"]:is(:hover, :focus-visible, [data-state="open"]) {
  color: var(--codeskin-content-foreground) !important;
  background-color: color-mix(in srgb, var(--codeskin-content-panel-color) calc(var(--codeskin-content-hover-opacity) * 100%), transparent) !important;
  border-color: color-mix(in srgb, var(--codeskin-content-foreground) 30%, transparent) !important;
}
/* The responsive utility sidebar can remain mounted over an existing transcript,
   so page mode may stay focus even though this real aside is visible. Target its
   semantic aside and exact surface tokens directly instead of relying on ambient
   mode or broad main-surface recolouring. */
:root[${ownerAttribute}] .main-surface aside[class~="z-[41]"] [class~="bg-token-main-surface-primary"] {
  color: var(--codeskin-content-foreground) !important;
  background-color: color-mix(in srgb, var(--codeskin-content-panel-color) calc(var(--codeskin-content-panel-opacity) * 100%), transparent) !important;
  border-color: color-mix(in srgb, var(--codeskin-content-foreground) 22%, transparent) !important;
  backdrop-filter: blur(var(--codeskin-content-blur)) saturate(112%);
  -webkit-backdrop-filter: blur(var(--codeskin-content-blur)) saturate(112%);
  text-shadow: var(--codeskin-content-text-shadow) !important;
}
:root[${ownerAttribute}] .main-surface aside[class~="z-[41]"] [class~="bg-token-main-surface-primary"] :is(
  [class*="text-token-text-primary"], [class*="text-token-foreground"]
) {
  color: var(--codeskin-content-foreground) !important;
  text-shadow: var(--codeskin-content-text-shadow) !important;
}
:root[${ownerAttribute}] .main-surface aside[class~="z-[41]"] [class~="bg-token-main-surface-primary"] :is(
  [class*="text-token-text-secondary"], [class*="text-token-text-tertiary"],
  [class*="text-token-description-foreground"]
) {
  color: var(--codeskin-content-muted) !important;
  text-shadow: var(--codeskin-content-text-shadow) !important;
}
:root[${ownerAttribute}] .main-surface aside[class~="z-[41]"] [class~="bg-token-bg-fog"] {
  color: var(--codeskin-content-foreground) !important;
  background-color: color-mix(in srgb, var(--codeskin-content-panel-color) calc(var(--codeskin-content-panel-opacity) * 100%), transparent) !important;
  border-color: color-mix(in srgb, var(--codeskin-content-foreground) 20%, transparent) !important;
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--codeskin-content-foreground) 8%, transparent) !important;
  text-shadow: var(--codeskin-content-text-shadow) !important;
}
:root[${ownerAttribute}] .main-surface aside[class~="z-[41]"] [class~="bg-token-bg-fog"]:is(:hover, :focus-visible, [data-state="open"]) {
  color: var(--codeskin-content-foreground) !important;
  background-color: color-mix(in srgb, var(--codeskin-content-panel-color) calc(var(--codeskin-content-hover-opacity) * 100%), transparent) !important;
  border-color: color-mix(in srgb, var(--codeskin-content-foreground) 30%, transparent) !important;
}
/* User messages expose a stable semantic marker. Do not select the Tailwind
   bg-token-foreground/5 fragment: it also appears inside hover utility class
   strings on unrelated header buttons and would recolour them accidentally. */
:root[${ownerAttribute}] [data-user-message-bubble="true"] {
  color: var(--codeskin-content-foreground) !important;
  background-color: color-mix(in srgb, var(--codeskin-content-panel-color) calc(var(--codeskin-content-panel-opacity) * 100%), transparent) !important;
  backdrop-filter: blur(var(--codeskin-content-blur)) saturate(112%);
  -webkit-backdrop-filter: blur(var(--codeskin-content-blur)) saturate(112%);
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--codeskin-content-foreground) 18%, transparent), 0 10px 28px color-mix(in srgb, black 24%, transparent) !important;
}
:root[${ownerAttribute}] [data-user-message-bubble="true"] :is(
  [class*="text-token-text"], [class*="_markdownContent_"], h1, h2, h3, h4, p, span, li, label, a, code
) {
  color: var(--codeskin-content-foreground) !important;
  text-shadow: var(--codeskin-content-text-shadow) !important;
}
/* Project and application menus are normally opened over the left/header area,
   so their portal glass follows the sidebar sample. It remains translucent and
   wallpaper-aware rather than switching to a fixed dark popup. */
:root[${ownerAttribute}] :is([role="menu"], [data-radix-menu-content]) {
  color: var(--codeskin-sidebar-foreground) !important;
  background-color: color-mix(in srgb, var(--codeskin-sidebar-panel-color) calc(var(--codeskin-sidebar-elevated-opacity) * 100%), transparent) !important;
  border-color: color-mix(in srgb, var(--codeskin-sidebar-foreground) 26%, transparent) !important;
  backdrop-filter: blur(var(--codeskin-sidebar-blur)) saturate(116%);
  -webkit-backdrop-filter: blur(var(--codeskin-sidebar-blur)) saturate(116%);
  box-shadow: 0 18px 46px color-mix(in srgb, var(--codeskin-sidebar-panel-color) 34%, transparent) !important;
}
:root[${ownerAttribute}] :is([role="menu"], [data-radix-menu-content]) :is(
  [role^="menuitem"], button, a, h1, h2, h3, h4, p, span, label, kbd, svg,
  input, textarea, [contenteditable="true"], [class*="text-token-text"], [class*="text-token-foreground"]
) {
  color: var(--codeskin-sidebar-foreground) !important;
  text-shadow: var(--codeskin-sidebar-text-shadow) !important;
}
:root[${ownerAttribute}] :is([role="menu"], [data-radix-menu-content]) :is(
  [class*="text-token-text-secondary"], [class*="text-token-text-tertiary"],
  [class*="text-token-description-foreground"]
) {
  color: var(--codeskin-sidebar-muted) !important;
}
:root[${ownerAttribute}] :is([role="dialog"], [role="listbox"], [data-radix-popover-content], [data-radix-select-content], [class*="bg-token-dropdown-background"]) {
  color: var(--codeskin-info-foreground) !important;
  background-color: color-mix(in srgb, var(--codeskin-info-panel-color) calc(var(--codeskin-info-elevated-opacity) * 100%), transparent) !important;
  border-color: color-mix(in srgb, var(--codeskin-info-foreground) 26%, transparent) !important;
  backdrop-filter: blur(var(--codeskin-info-blur)) saturate(116%);
  -webkit-backdrop-filter: blur(var(--codeskin-info-blur)) saturate(116%);
  box-shadow: 0 18px 46px color-mix(in srgb, var(--codeskin-info-panel-color) 34%, transparent) !important;
}
:root[${ownerAttribute}] :is(
  [role="dialog"], [role="listbox"], [data-radix-popover-content],
  [data-radix-select-content], [class*="bg-token-dropdown-background"]
) :is(
  button, a, h1, h2, h3, h4, p, span, label, kbd, svg,
  input, textarea, [contenteditable="true"], [class*="text-token-text"], [class*="text-token-foreground"]
) {
  color: var(--codeskin-info-foreground) !important;
  text-shadow: var(--codeskin-info-text-shadow) !important;
}
:root[${ownerAttribute}] :is(
  [role="dialog"], [role="listbox"], [data-radix-popover-content],
  [data-radix-select-content], [class*="bg-token-dropdown-background"]
) :is(
  [class*="text-token-text-secondary"], [class*="text-token-text-tertiary"],
  [class*="text-token-description-foreground"]
) {
  color: var(--codeskin-info-muted) !important;
}
:root[${ownerAttribute}] :is([role="menuitem"], [role="menuitemcheckbox"], [role="menuitemradio"]):is(:hover, :focus-visible, [data-highlighted]) {
  color: var(--codeskin-sidebar-foreground) !important;
  background-color: color-mix(in srgb, var(--codeskin-sidebar-foreground) 13%, transparent) !important;
}
/* The composer keeps its own sampled glass in both welcome and task views. */
:root[${ownerAttribute}] .composer-surface-chrome {
  color: var(--codeskin-composer-foreground) !important;
  background-color: color-mix(in srgb, var(--codeskin-composer-panel-color) calc(var(--codeskin-composer-panel-opacity) * 100%), transparent) !important;
  border-color: color-mix(in srgb, var(--codeskin-composer-muted) 30%, transparent) !important;
  backdrop-filter: blur(var(--codeskin-composer-blur)) saturate(114%);
  -webkit-backdrop-filter: blur(var(--codeskin-composer-blur)) saturate(114%);
  box-shadow: 0 14px 38px color-mix(in srgb, var(--codeskin-composer-panel-color) 28%, transparent) !important;
}
:root[${ownerAttribute}] .composer-surface-chrome :is(
  [class*="text-token-text"], h1, h2, h3, p, span, label, input, textarea, [contenteditable="true"]
) {
  color: var(--codeskin-composer-foreground) !important;
  text-shadow: var(--codeskin-composer-text-shadow) !important;
}
:root[${ownerAttribute}] .app-shell-left-panel ::placeholder,
:root[${ownerAttribute}] :is([role="menu"], [data-radix-menu-content]) ::placeholder {
  color: var(--codeskin-sidebar-muted) !important;
  opacity: 1 !important;
}
:root[${ownerAttribute}] .composer-surface-chrome ::placeholder {
  color: var(--codeskin-composer-muted) !important;
  opacity: 1 !important;
}
:root[${ownerAttribute}] :is([role="dialog"], [role="listbox"], [data-radix-popover-content], [data-radix-select-content]) ::placeholder {
  color: var(--codeskin-info-muted) !important;
  opacity: 1 !important;
}
:root[${ownerAttribute}][${modeAttribute}="ambient"] .main-surface ::placeholder {
  color: var(--codeskin-content-muted) !important;
  opacity: 1 !important;
}
:root[${ownerAttribute}] .app-shell-left-panel :disabled,
:root[${ownerAttribute}] :is([role="menu"], [data-radix-menu-content]) :disabled {
  color: var(--codeskin-sidebar-muted) !important;
  opacity: 0.82 !important;
}
:root[${ownerAttribute}] .composer-surface-chrome :disabled {
  color: var(--codeskin-composer-muted) !important;
  opacity: 0.82 !important;
}
:root[${ownerAttribute}] :is([role="dialog"], [role="listbox"], [data-radix-popover-content], [data-radix-select-content]) :disabled {
  color: var(--codeskin-info-muted) !important;
  opacity: 0.82 !important;
}
:root[${ownerAttribute}][${modeAttribute}="ambient"] .main-surface :disabled {
  color: var(--codeskin-content-muted) !important;
  opacity: 0.82 !important;
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
