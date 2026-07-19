import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";
import { fileURLToPath } from "node:url";

const thisDirectory = path.dirname(fileURLToPath(import.meta.url));
const installSource = fs.readFileSync(path.join(thisDirectory, "../injection-js/install.js"), "utf8");
const restoreSource = fs.readFileSync(path.join(thisDirectory, "../injection-js/restore.js"), "utf8");
const verifySource = fs.readFileSync(path.join(thisDirectory, "../injection-js/verify.js"), "utf8");

const createRoot = () => {
  const attributes = new Map();
  const mutations = [];
  return {
    attributes,
    mutations,
    getAttribute(name) {
      return attributes.get(name) ?? null;
    },
    setAttribute(name, value) {
      mutations.push({ operation: "set", name, value });
      attributes.set(name, value);
    },
    removeAttribute(name) {
      mutations.push({ operation: "remove", name });
      attributes.delete(name);
    }
  };
};

const createDocument = () => {
  const listeners = new Map();
  return {
    documentElement: null,
    body: null,
    getElementById() {
      return null;
    },
    addEventListener(type, listener, options) {
      const registered = listeners.get(type) ?? [];
      registered.push({ listener, once: Boolean(options && options.once) });
      listeners.set(type, registered);
    },
    removeEventListener(type, listener) {
      const registered = listeners.get(type) ?? [];
      listeners.set(type, registered.filter((entry) => entry.listener !== listener));
    },
    dispatch(type) {
      const registered = [...(listeners.get(type) ?? [])];
      for (const entry of registered) {
        if (entry.once) this.removeEventListener(type, entry.listener);
        entry.listener();
      }
    },
    listenerCount(type) {
      return (listeners.get(type) ?? []).length;
    }
  };
};

const createLiveDocument = () => {
  const document = createDocument();
  const connectedNodes = [];
  const connect = (node) => {
    if (!connectedNodes.includes(node)) connectedNodes.push(node);
    node.isConnected = true;
    node.parentNode = null;
    return node;
  };
  const container = () => ({
    firstChild: null,
    appendChild(node) {
      this.firstChild ??= node;
      return connect(node);
    },
    insertBefore(node, before) {
      if (before === this.firstChild) this.firstChild = node;
      else this.firstChild ??= node;
      return connect(node);
    }
  });

  document.documentElement = createRoot();
  document.head = container();
  document.body = container();
  document.createElement = (tagName) => {
    const attributes = new Map();
    const node = {
      nodeType: 1,
      tagName: tagName.toUpperCase(),
      id: "",
      isConnected: false,
      parentNode: null,
      style: {},
      textContent: "",
      getAttribute(name) {
        return attributes.get(name) ?? null;
      },
      setAttribute(name, value) {
        attributes.set(name, value);
      },
      remove() {
        this.isConnected = false;
      }
    };
    return node;
  };
  document.getElementById = (id) => connectedNodes.find((node) => node.isConnected && node.id === id) ?? null;
  document.querySelector = () => null;
  return document;
};

const createMutationObserver = () => class MutationObserver {
  constructor(callback) {
    this.callback = callback;
    this.disconnectCalls = 0;
    this.observed = [];
  }

  observe(target, options) {
    this.observed.push({ target, options });
  }

  disconnect() {
    this.disconnectCalls += 1;
  }
};

const createLiveContext = () => {
  const document = createLiveDocument();
  const window = {};
  const context = vm.createContext({
    document,
    window,
    URL,
    console,
    MutationObserver: createMutationObserver(),
    queueMicrotask: (callback) => callback(),
    getComputedStyle: () => ({ getPropertyValue: () => "#7AA2F7" })
  });
  return { document, window, context };
};

const run = (source, context) => vm.runInContext(source, context);
const theme = {
  id: "tokyo-night",
  colors: {
    accent: "#7AA2F7",
    secondary: "#BB9AF7",
    background: "#1A1B26",
    surface: "#24283B",
    foreground: "#C0CAF5",
    muted: "#565F89"
  },
  backgroundImage: "data:image/jpeg;base64,/9j/2Q==",
  layers: {},
  contrast: {
    sidebar: {
      foreground: "#F4F7FF", muted: "#C7D0DC", panelColor: "#12161D",
      panelOpacity: 0.37, blurPx: 14, textShadow: "0 1px 2px rgba(0,0,0,0.4)"
    },
    content: {
      foreground: "#172033", muted: "#536174", panelColor: "#F7F4EE",
      panelOpacity: 0.18, blurPx: 8, textShadow: "0 1px 2px rgba(255,255,255,0.44)"
    },
    header: {
      foreground: "#263744", muted: "#52616D", panelColor: "#F7F4EE",
      panelOpacity: 0.21, blurPx: 9, textShadow: "0 1px 2px rgba(255,255,255,0.4)"
    },
    infoPanel: {
      foreground: "#F4F7FF", muted: "#C7D0DC", panelColor: "#12161D",
      panelOpacity: 0.39, blurPx: 15, textShadow: "0 1px 2px rgba(0,0,0,0.4)"
    },
    composer: {
      foreground: "#F4F7FF", muted: "#C7D0DC", panelColor: "#12161D",
      panelOpacity: 0.33, blurPx: 13, textShadow: "0 1px 2px rgba(0,0,0,0.4)"
    }
  }
};

const pendingRuntimeWithNoRootIsRemovedBeforeDOMContentLoaded = () => {
  const document = createDocument();
  const window = {};
  const context = vm.createContext({
    document,
    window,
    URL,
    console,
    getComputedStyle: () => ({ getPropertyValue: () => "" })
  });
  const install = run(`(${installSource})`, context);

  const result = install({});
  assert.equal(result.active, false);
  assert.equal(result.pending, true);
  assert.equal(window.__codeskinRuntime.root, null);
  let unrelatedHandlerCalls = 0;
  document.addEventListener("DOMContentLoaded", () => {
    unrelatedHandlerCalls += 1;
  });
  assert.equal(document.listenerCount("DOMContentLoaded"), 2);

  const root = createRoot();
  document.documentElement = root;
  assert.equal(document.body, null);

  assert.equal(run(restoreSource, context).active, false);
  assert.equal(document.listenerCount("DOMContentLoaded"), 1);
  assert.equal("__codeskinRuntime" in window, false);
  assert.equal(root.attributes.has("data-codeskin-theme-id"), false);
  assert.equal(root.attributes.has("data-codeskin-mode"), false);
  assert.deepEqual(root.mutations, []);

  document.dispatch("DOMContentLoaded");
  assert.equal(unrelatedHandlerCalls, 1);
  assert.equal(document.getElementById("codeskin-runtime-style"), null);
  assert.equal(document.getElementById("codeskin-wallpaper-layer"), null);
  assert.equal(root.attributes.has("data-codeskin-theme-id"), false);
  assert.equal(root.attributes.has("data-codeskin-mode"), false);
  assert.deepEqual(root.mutations, []);
  assert.equal("__codeskinRuntime" in window, false);
};

const foreignPendingRuntimeIsNotRemoved = () => {
  const document = createDocument();
  const root = createRoot();
  document.documentElement = root;
  const window = {};
  let foreignHandlerCalls = 0;
  const foreignPendingInstall = () => {
    foreignHandlerCalls += 1;
  };
  const foreignRuntime = {
    owner: "foreign-runtime",
    version: 1,
    root: null,
    style: null,
    wallpaper: null,
    observer: null,
    pendingInstall: foreignPendingInstall,
    modeUpdateQueued: false
  };
  window.__codeskinRuntime = foreignRuntime;
  document.addEventListener("DOMContentLoaded", foreignPendingInstall, { once: true });

  const context = vm.createContext({ document, window, console });
  assert.equal(run(restoreSource, context).active, false);
  assert.equal(window.__codeskinRuntime, foreignRuntime);
  assert.equal(document.listenerCount("DOMContentLoaded"), 1);
  assert.deepEqual(root.mutations, []);

  document.dispatch("DOMContentLoaded");
  assert.equal(foreignHandlerCalls, 1);
  assert.equal(window.__codeskinRuntime, foreignRuntime);
};

const installExposesObserverAndRestoreRemovesIt = () => {
  const { window, context } = createLiveContext();
  const install = run(`(${installSource})`, context);

  assert.equal(install(theme).active, true);
  const observer = window.__codeskinRuntime.observer;
  assert.equal(window.__codeskinModeObserver, observer);

  assert.equal(run(restoreSource, context).active, false);
  assert.equal(observer.disconnectCalls, 1);
  assert.equal("__codeskinModeObserver" in window, false);
  assert.equal("__codeskinRuntime" in window, false);
};

const foreignObserverRemainsAndInstallFailsClosed = () => {
  const { document, window, context } = createLiveContext();
  const install = run(`(${installSource})`, context);
  const foreignObserver = {
    disconnectCalls: 0,
    disconnect() {
      this.disconnectCalls += 1;
    }
  };
  window.__codeskinModeObserver = foreignObserver;

  const result = install(theme);
  assert.equal(result.active, false);
  assert.equal(result.reason, "observer-conflict");
  assert.equal(window.__codeskinModeObserver, foreignObserver);
  assert.equal(foreignObserver.disconnectCalls, 0);
  assert.equal("__codeskinRuntime" in window, false);
  assert.equal(document.getElementById("codeskin-runtime-style"), null);
  assert.equal(document.getElementById("codeskin-wallpaper-layer"), null);
};

const repeatInstallReplacesOnlyItsOwnObserver = () => {
  const { window, context } = createLiveContext();
  const install = run(`(${installSource})`, context);

  assert.equal(install(theme).active, true);
  const firstObserver = window.__codeskinModeObserver;
  assert.equal(install(theme).active, true);
  const secondObserver = window.__codeskinModeObserver;

  assert.notEqual(secondObserver, firstObserver);
  assert.equal(firstObserver.disconnectCalls, 1);
  assert.equal(secondObserver.disconnectCalls, 0);
  assert.equal(window.__codeskinRuntime.observer, secondObserver);
};

const restoreDoesNotDeleteOrDisconnectAForeignObserver = () => {
  const { window, context } = createLiveContext();
  const install = run(`(${installSource})`, context);

  assert.equal(install(theme).active, true);
  const codeskinObserver = window.__codeskinModeObserver;
  const foreignObserver = {
    disconnectCalls: 0,
    disconnect() {
      this.disconnectCalls += 1;
    }
  };
  window.__codeskinModeObserver = foreignObserver;

  assert.equal(run(restoreSource, context).active, false);
  assert.equal(codeskinObserver.disconnectCalls, 1);
  assert.equal(foreignObserver.disconnectCalls, 0);
  assert.equal(window.__codeskinModeObserver, foreignObserver);
};

const verifyFailsClosedWhenObserverGlobalDoesNotMatchRuntime = () => {
  const { window, context } = createLiveContext();
  const install = run(`(${installSource})`, context);

  assert.equal(install(theme).active, true);
  assert.equal(run(verifySource, context).active, true);
  window.__codeskinModeObserver = { disconnect() {} };

  const verification = run(verifySource, context);
  assert.equal(verification.active, false);
  assert.equal(verification.safe, false);
};


const cssRule = (css, selector) => {
  const start = css.indexOf(`${selector} {`);
  assert.notEqual(start, -1, `expected injected rule for ${selector}`);
  const end = css.indexOf("\n}", start);
  assert.notEqual(end, -1, `expected closing brace for ${selector}`);
  return css.slice(start, end + 2);
};

const hexToRgb = (hex) => [1, 3, 5].map((index) => Number.parseInt(hex.slice(index, index + 2), 16));
const blendRgb = (foreground, background, opacity) => foreground.map(
  (channel, index) => Math.round(channel * opacity + background[index] * (1 - opacity))
);
const relativeLuminance = (rgb) => {
  const linear = rgb.map((channel) => {
    const value = channel / 255;
    return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2];
};
const contrastRatio = (left, right) => {
  const l1 = relativeLuminance(left);
  const l2 = relativeLuminance(right);
  return (Math.max(l1, l2) + 0.05) / (Math.min(l1, l2) + 0.05);
};
const cssVariable = (css, name) => {
  const match = css.match(new RegExp(`--${name}:\\s*([^;]+);`));
  assert.ok(match, `expected CSS variable --${name}`);
  return match[1].trim();
};

const injectedSurfacesUseRegionalGlassWithoutGlassingTranscript = () => {
  const { document, context } = createLiveContext();
  const install = run(`(${installSource})`, context);

  assert.equal(install(theme).active, true);
  const css = document.getElementById("codeskin-runtime-style").textContent;
  const wallpaperVeilRule = cssRule(
    css,
    '#codeskin-wallpaper-layer[data-codeskin-owned="true"][data-codeskin-layer="wallpaper"][data-codeskin-runtime="codeskin-runtime-v1"]::after'
  );
  const mainSurfaceRule = cssRule(css, ":root[data-codeskin-theme-id] .main-surface");
  const sidebarRule = cssRule(css, ":root[data-codeskin-theme-id] .app-shell-left-panel");
  const composerRule = cssRule(css, ":root[data-codeskin-theme-id] .composer-surface-chrome");

  assert.equal(cssVariable(css, "codeskin-ambient-overlay-opacity"), "0.12");
  assert.equal(cssVariable(css, "codeskin-focus-overlay-opacity"), "0.18");
  assert.equal(cssVariable(css, "codeskin-sidebar-panel-color"), theme.contrast.sidebar.panelColor);
  assert.equal(cssVariable(css, "codeskin-sidebar-panel-opacity"), String(theme.contrast.sidebar.panelOpacity));
  assert.equal(cssVariable(css, "codeskin-sidebar-blur"), `${theme.contrast.sidebar.blurPx}px`);
  assert.equal(cssVariable(css, "codeskin-content-panel-color"), theme.contrast.content.panelColor);
  assert.equal(cssVariable(css, "codeskin-content-panel-opacity"), String(theme.contrast.content.panelOpacity));
  assert.equal(cssVariable(css, "codeskin-content-text-shadow"), theme.contrast.content.textShadow);
  assert.equal(cssVariable(css, "codeskin-info-panel-color"), theme.contrast.infoPanel.panelColor);
  assert.equal(cssVariable(css, "codeskin-composer-panel-color"), theme.contrast.composer.panelColor);
  assert.equal(cssVariable(css, "codeskin-composer-panel-opacity"), String(theme.contrast.composer.panelOpacity));
  assert.doesNotMatch(css, /#11151C|--codeskin-glass-color|--codeskin-glass-base-opacity/);
  assert.ok(Number(cssVariable(css, "codeskin-ambient-overlay-opacity")) <= 0.14);
  assert.ok(Number(cssVariable(css, "codeskin-focus-overlay-opacity")) <= 0.22);
  assert.match(wallpaperVeilRule, /opacity: var\(--codeskin-current-overlay-opacity\);/);
  assert.match(wallpaperVeilRule, /background: var\(--codeskin-wallpaper-veil\);/);

  assert.match(mainSurfaceRule, /background: transparent !important;/);
  assert.match(mainSurfaceRule, /backdrop-filter: none !important;/);
  assert.doesNotMatch(mainSurfaceRule, /backdrop-filter: blur/);
  assert.match(sidebarRule, /var\(--codeskin-sidebar-panel-color\)/);
  assert.match(sidebarRule, /var\(--codeskin-sidebar-panel-opacity\)/);
  assert.match(sidebarRule, /backdrop-filter: blur\(var\(--codeskin-sidebar-blur\)\)/);
  assert.match(sidebarRule, /color: var\(--codeskin-sidebar-foreground\) !important;/);
  assert.match(composerRule, /var\(--codeskin-composer-panel-color\)/);
  assert.match(composerRule, /var\(--codeskin-composer-panel-opacity\)/);
  assert.match(composerRule, /backdrop-filter: blur\(var\(--codeskin-composer-blur\)\)/);
  assert.match(composerRule, /color: var\(--codeskin-composer-foreground\) !important;/);

  assert.match(
    css,
    /\[class\*="_markdownContent_"\][\s\S]*?background: transparent !important;/,
    "chat and code wrappers must stay transparent"
  );
  assert.doesNotMatch(
    css,
    /\.main-surface\s*\{[^}]*--codeskin-(?:content|info|sidebar)-panel-opacity/s,
    "the main transcript surface must not become one giant glass panel"
  );
};

const elevatedMenusDialogsAndSettingsUseRegionalReadableGlass = () => {
  const { document, context } = createLiveContext();
  const install = run(`(${installSource})`, context);

  assert.equal(install(theme).active, true);
  const css = document.getElementById("codeskin-runtime-style").textContent;
  const menuRule = cssRule(
    css,
    ':root[data-codeskin-theme-id] :is([role="menu"], [data-radix-menu-content])'
  );
  const elevatedRule = cssRule(
    css,
    ':root[data-codeskin-theme-id] :is([role="dialog"], [role="listbox"], [data-radix-popover-content], [data-radix-select-content], [class*="bg-token-dropdown-background"])'
  );
  const menuItemRule = cssRule(
    css,
    ':root[data-codeskin-theme-id] :is([role="menuitem"], [role="menuitemcheckbox"], [role="menuitemradio"]):is(:hover, :focus-visible, [data-highlighted])'
  );
  const ambientCardRule = cssRule(
    css,
    ':root[data-codeskin-theme-id][data-codeskin-mode="ambient"] .main-surface [class~="group/home-suggestions"] button[class*="bg-token-main-surface-primary"]'
  );
  const settingsRule = cssRule(
    css,
    ':root[data-codeskin-theme-id][data-codeskin-mode="ambient"] .main-surface :is([class*="bg-token-main-surface-primary"], [class*="bg-token-main-surface-secondary"], [class*="bg-token-dropdown-background"])'
  );

  assert.match(menuRule, /var\(--codeskin-sidebar-panel-color\)/);
  assert.match(menuRule, /var\(--codeskin-sidebar-elevated-opacity\)/);
  assert.match(menuRule, /backdrop-filter: blur\(var\(--codeskin-sidebar-blur\)\)/);
  assert.match(menuRule, /color: var\(--codeskin-sidebar-foreground\) !important;/);
  assert.match(elevatedRule, /var\(--codeskin-info-panel-color\)/);
  assert.match(elevatedRule, /var\(--codeskin-info-elevated-opacity\)/);
  assert.match(elevatedRule, /backdrop-filter: blur\(var\(--codeskin-info-blur\)\)/);
  assert.match(elevatedRule, /color: var\(--codeskin-info-foreground\) !important;/);
  assert.match(menuItemRule, /background-color: color-mix/);
  assert.match(menuItemRule, /transparent\) !important;/);
  assert.match(ambientCardRule, /var\(--codeskin-content-panel-opacity\)/);
  assert.match(ambientCardRule, /color: var\(--codeskin-content-foreground\) !important;/);
  assert.match(settingsRule, /var\(--codeskin-content-panel-opacity\)/);
  assert.match(settingsRule, /color: var\(--codeskin-content-foreground\) !important;/);
  assert.match(css, /::placeholder[\s\S]*?color: var\(--codeskin-[a-z-]+-muted\) !important;/);
  assert.match(css, /:disabled[\s\S]*?color: var\(--codeskin-[a-z-]+-muted\) !important;/);
};

const settingsCardsAndFogControlsUseRegionalAmbientGlass = () => {
  const { document, context } = createLiveContext();
  const install = run(`(${installSource})`, context);

  assert.equal(install(theme).active, true);
  const css = document.getElementById("codeskin-runtime-style").textContent;
  const settingsCardSelector = ':root[data-codeskin-theme-id][data-codeskin-mode="ambient"] .main-surface [class~="overflow-hidden"][class~="rounded-2xl"][class~="border-token-border"]';
  const settingsCardRule = cssRule(css, settingsCardSelector);
  const settingsDividerRule = cssRule(css, `${settingsCardSelector} > *::after`);
  const fogControlRule = cssRule(
    css,
    ':root[data-codeskin-theme-id][data-codeskin-mode="ambient"] .main-surface [class~="bg-token-bg-fog"]'
  );
  const fogControlInteractiveRule = cssRule(
    css,
    ':root[data-codeskin-theme-id][data-codeskin-mode="ambient"] .main-surface [class~="bg-token-bg-fog"]:is(:hover, :focus-visible, [data-state="open"])'
  );

  assert.match(settingsCardRule, /color: var\(--codeskin-content-foreground\) !important;/);
  assert.match(settingsCardRule, /var\(--codeskin-content-panel-color\)/);
  assert.match(settingsCardRule, /var\(--codeskin-content-panel-opacity\)/);
  assert.match(settingsCardRule, /backdrop-filter: blur\(var\(--codeskin-content-blur\)\)/);
  assert.match(settingsCardRule, /border-color: color-mix/);
  assert.match(settingsCardRule, /box-shadow:/);
  assert.doesNotMatch(settingsCardRule, /background(?:-color)?:\s*(?:rgb\(250,\s*250,\s*248\)|#[fF][aA][fF][aA][fF][8])/);

  assert.match(settingsDividerRule, /background-color: color-mix/);
  assert.match(settingsDividerRule, /var\(--codeskin-content-foreground\)/);

  assert.match(fogControlRule, /color: var\(--codeskin-content-foreground\) !important;/);
  assert.match(fogControlRule, /var\(--codeskin-content-panel-opacity\)/);
  assert.match(fogControlRule, /border-color: color-mix/);
  assert.match(fogControlInteractiveRule, /var\(--codeskin-content-hover-opacity\)/);
  assert.doesNotMatch(fogControlInteractiveRule, /(?:white|#fff|rgb\(250,\s*250,\s*248\))/i);
};

const responsiveUtilitySidebarUsesContentGlassInFocusAndAmbientModes = () => {
  const { document, context } = createLiveContext();
  const install = run(`(${installSource})`, context);

  assert.equal(install(theme).active, true);
  const css = document.getElementById("codeskin-runtime-style").textContent;
  const sidebarSurfaceSelector = ':root[data-codeskin-theme-id] .main-surface aside[class~="z-[41]"] [class~="bg-token-main-surface-primary"]';
  const sidebarSurfaceRule = cssRule(css, sidebarSurfaceSelector);
  const shortcutSelector = ':root[data-codeskin-theme-id] .main-surface aside[class~="z-[41]"] [class~="bg-token-bg-fog"]';
  const shortcutRule = cssRule(css, shortcutSelector);
  const shortcutInteractiveRule = cssRule(
    css,
    `${shortcutSelector}:is(:hover, :focus-visible, [data-state="open"])`
  );

  assert.doesNotMatch(sidebarSurfaceSelector, /data-codeskin-mode/, "utility sidebar glass must also apply while an underlying transcript keeps focus mode active");
  assert.match(sidebarSurfaceRule, /color: var\(--codeskin-content-foreground\) !important;/);
  assert.match(sidebarSurfaceRule, /var\(--codeskin-content-panel-color\)/);
  assert.match(sidebarSurfaceRule, /var\(--codeskin-content-panel-opacity\)/);
  assert.match(sidebarSurfaceRule, /backdrop-filter: blur\(var\(--codeskin-content-blur\)\)/);
  assert.match(sidebarSurfaceRule, /text-shadow: var\(--codeskin-content-text-shadow\) !important;/);
  assert.match(shortcutRule, /color: var\(--codeskin-content-foreground\) !important;/);
  assert.match(shortcutRule, /var\(--codeskin-content-panel-opacity\)/);
  assert.match(shortcutRule, /text-shadow: var\(--codeskin-content-text-shadow\) !important;/);
  assert.match(shortcutInteractiveRule, /var\(--codeskin-content-hover-opacity\)/);
  assert.doesNotMatch(shortcutRule, /(?:white|#fff|rgb\(250,\s*250,\s*248\))/i);
};

const userMessageBubblesUseContentGlassWithoutMatchingHoverUtilities = () => {
  const { document, context } = createLiveContext();
  const install = run(`(${installSource})`, context);

  assert.equal(install(theme).active, true);
  const css = document.getElementById("codeskin-runtime-style").textContent;
  const bubbleRule = cssRule(
    css,
    ':root[data-codeskin-theme-id] [data-user-message-bubble="true"]'
  );

  assert.match(bubbleRule, /color: var\(--codeskin-content-foreground\) !important;/);
  assert.match(bubbleRule, /var\(--codeskin-content-panel-color\)/);
  assert.match(bubbleRule, /var\(--codeskin-content-panel-opacity\)/);
  assert.match(bubbleRule, /backdrop-filter: blur\(var\(--codeskin-content-blur\)\)/);
  assert.doesNotMatch(bubbleRule, /background: transparent !important;/);
  assert.doesNotMatch(
    css,
    /:root\[data-codeskin-theme-id\] \[class\\\*="bg-token-foreground\\\/5"\]/,
    "hover utility class fragments must not be mistaken for glass surfaces"
  );
};

const regionalGlassTextMeetsContrastAgainstItsSampledWallpaperArea = () => {
  const { document, context } = createLiveContext();
  const install = run(`(${installSource})`, context);

  assert.equal(install(theme).active, true);
  const css = document.getElementById("codeskin-runtime-style").textContent;
  assert.doesNotMatch(
    css,
    /text-shadow: 0 1px 3px rgba\(0,0,0,0\.72\)/,
    "content text shadows must follow the sampled region instead of forcing a dark-only shadow"
  );
  const veil = hexToRgb(cssVariable(css, "codeskin-wallpaper-veil"));
  const overlayOpacity = Number(cssVariable(css, "codeskin-ambient-overlay-opacity"));
  const regions = {
    sidebar: { sample: [35, 41, 47], cssPrefix: "sidebar" },
    content: { sample: [242, 233, 220], cssPrefix: "content" },
    infoPanel: { sample: [35, 41, 47], cssPrefix: "info" },
    composer: { sample: [35, 41, 47], cssPrefix: "composer" }
  };

  for (const [name, { sample, cssPrefix }] of Object.entries(regions)) {
    const region = theme.contrast[name];
    const foreground = hexToRgb(cssVariable(css, `codeskin-${cssPrefix}-foreground`));
    const muted = hexToRgb(cssVariable(css, `codeskin-${cssPrefix}-muted`));
    const panel = hexToRgb(cssVariable(css, `codeskin-${cssPrefix}-panel-color`));
    const panelOpacity = Number(cssVariable(css, `codeskin-${cssPrefix}-panel-opacity`));
    const veiled = blendRgb(veil, sample, overlayOpacity);
    const composite = blendRgb(panel, veiled, panelOpacity);

    assert.equal(cssVariable(css, `codeskin-${cssPrefix}-foreground`), region.foreground);
    assert.equal(cssVariable(css, `codeskin-${cssPrefix}-muted`), region.muted);
    assert.ok(
      contrastRatio(foreground, composite) >= 4.5,
      `${name} normal text must reach 4.5:1 against its sampled glass composite`
    );
    assert.ok(
      contrastRatio(muted, composite) >= 3,
      `${name} muted text must reach 3:1 against its sampled glass composite`
    );
  }
};

const currentTaskHeaderTitleUsesReadableSampledHeaderText = () => {
  const { document, context } = createLiveContext();
  const install = run(`(${installSource})`, context);

  assert.equal(install(theme).active, true);
  const css = document.getElementById("codeskin-runtime-style").textContent;
  const titleRule = cssRule(
    css,
    ':root[data-codeskin-theme-id] [data-testid="app-shell-header-context-menu-surface"] :is([class~="text-token-foreground"], [class~="text-token-foreground"] *)'
  );
  const sidebarVisibilityToggleSelector = ':root[data-codeskin-theme-id] .app-header-tint button[aria-pressed][class~="aspect-square"][class~="text-token-foreground"][class~="bg-token-foreground/5"]';
  const sidebarVisibilityToggleRule = cssRule(css, sidebarVisibilityToggleSelector);
  const sidebarVisibilityToggleInteractiveRule = cssRule(
    css,
    `${sidebarVisibilityToggleSelector}:is(:hover, :focus-visible, [aria-pressed="true"])`
  );

  assert.match(titleRule, /color: var\(--codeskin-header-foreground\) !important;/);
  assert.match(titleRule, /text-shadow: var\(--codeskin-header-text-shadow\) !important;/);
  assert.doesNotMatch(sidebarVisibilityToggleSelector, /aria-label/, "selector must remain locale-independent");
  assert.match(sidebarVisibilityToggleRule, /color: var\(--codeskin-header-icon-foreground\) !important;/);
  assert.match(sidebarVisibilityToggleRule, /var\(--codeskin-header-foreground\)/);
  assert.match(sidebarVisibilityToggleRule, /text-shadow: var\(--codeskin-header-text-shadow\) !important;/);
  assert.match(sidebarVisibilityToggleInteractiveRule, /var\(--codeskin-header-foreground\)/);
  assert.doesNotMatch(sidebarVisibilityToggleRule, /(?:white|#fff|rgb\(250,\s*250,\s*248\))/i);
};

const openLocationButtonGroupUsesSampledHeaderGlassWithoutLocaleSelectors = () => {
  const { document, context } = createLiveContext();
  const install = run(`(${installSource})`, context);

  assert.equal(install(theme).active, true);
  const css = document.getElementById("codeskin-runtime-style").textContent;
  const buttonSelector = ':root[data-codeskin-theme-id] [data-testid="app-shell-header-context-menu-surface"] div[class~="inline-flex"][class~="items-stretch"][class~="overflow-hidden"][class~="rounded-lg"] button[class~="border-token-border"][class~="text-token-button-tertiary-foreground"][class~="bg-token-bg-fog"]:is([class~="rounded-r-none"], [class~="rounded-l-none"])';
  const buttonRule = cssRule(css, buttonSelector);
  const interactiveRule = cssRule(css, `${buttonSelector}:is(:hover, :focus-visible, [data-state="open"])`);
  const arrowRule = cssRule(css, `${buttonSelector} svg[class~="opacity-50"]`);
  assert.notEqual(css.indexOf(`@layer base {\n${buttonSelector} {`), -1, "layered rule must beat Codex !border-token-border");

  assert.equal(cssVariable(css, "codeskin-header-panel-color"), theme.contrast.header.panelColor);
  assert.equal(cssVariable(css, "codeskin-header-panel-opacity"), String(theme.contrast.header.panelOpacity));
  assert.equal(cssVariable(css, "codeskin-header-blur"), `${theme.contrast.header.blurPx}px`);
  assert.doesNotMatch(buttonSelector, /data-codeskin-mode|aria-label|打开位置|次要操作/);
  assert.match(buttonRule, /color: var\(--codeskin-header-foreground\) !important;/);
  assert.match(buttonRule, /var\(--codeskin-header-panel-color\)/);
  assert.match(buttonRule, /var\(--codeskin-header-panel-opacity\)/);
  assert.match(buttonRule, /backdrop-filter: blur\(var\(--codeskin-header-blur\)\)/);
  assert.match(buttonRule, /text-shadow: var\(--codeskin-header-text-shadow\) !important;/);
  assert.match(interactiveRule, /var\(--codeskin-header-hover-opacity\)/);
  assert.match(arrowRule, /color: var\(--codeskin-header-foreground\) !important;/);
  assert.match(arrowRule, /opacity: 0\.82 !important;/);
  assert.doesNotMatch(buttonRule, /(?:white|#fff|rgb\(250,\s*250,\s*248\))/i);
  assert.doesNotMatch(css, /\.main-surface\s+svg\s*\{/);
};

const activitySummaryTrailingUsesReadableMutedColorWithoutGlobalSvgOverride = () => {
  const { document, context } = createLiveContext();
  const install = run(`(${installSource})`, context);

  assert.equal(install(theme).active, true);
  const css = document.getElementById("codeskin-runtime-style").textContent;
  const summaryRule = cssRule(
    css,
    ':root[data-codeskin-theme-id] body #root .main-surface [class~="group/activity-header"] [class*="text-token-conversation-summary"]'
  );
  const currentProcessedStatusRule = cssRule(
    css,
    ':root[data-codeskin-theme-id] body #root .main-surface button[aria-expanded] [class~="text-token-conversation-body"]'
  );

  assert.match(
    css,
    /@layer base \{[\s\S]*?:root\[data-codeskin-theme-id\] body #root \.main-surface \[class~="group\/activity-header"\] \[class\*="text-token-conversation-summary"\]/
  );
  assert.match(summaryRule, /color: var\(--codeskin-content-muted\) !important;/);
  assert.match(summaryRule, /text-shadow: var\(--codeskin-content-text-shadow\) !important;/);
  assert.match(currentProcessedStatusRule, /color: var\(--codeskin-content-muted\) !important;/);
  assert.match(currentProcessedStatusRule, /text-shadow: var\(--codeskin-content-text-shadow\) !important;/);
  assert.match(
    css,
    /\[class~="group\/activity-header"\] :is\(\[class\*="text-token-conversation-body"\], \[class\*="text-token-conversation-body"\] :is\(span, p, label, svg\)\)[\s\S]*?color: var\(--codeskin-content-muted\) !important;/,
    "nested activity text and currentColor icons must not fall back to Codex dark utility colours"
  );
  assert.doesNotMatch(
    css,
    /:root\[data-codeskin-theme-id\] \.main-surface svg\s*\{/,
    "activity icon readability must not introduce a global SVG override"
  );
};

const directWallpaperTextUsesSampledRegionalContrastWithLightOverlays = () => {
  const { document, context } = createLiveContext();
  const install = run(`(${installSource})`, context);

  assert.equal(install(theme).active, true);
  const css = document.getElementById("codeskin-runtime-style").textContent;
  const veil = hexToRgb(cssVariable(css, "codeskin-wallpaper-veil"));
  const overlays = {
    ambient: Number(cssVariable(css, "codeskin-ambient-overlay-opacity")),
    focus: Number(cssVariable(css, "codeskin-focus-overlay-opacity"))
  };
  const regions = {
    content: { sample: [242, 233, 220], prefix: "content" },
    header: { sample: [242, 233, 220], prefix: "header", mutedName: "header-muted-foreground" }
  };

  assert.ok(overlays.ambient <= 0.14, "ambient overlay must remain visually light");
  assert.ok(overlays.focus <= 0.22, "focus overlay must remain visually light");
  for (const [name, { sample, prefix, mutedName }] of Object.entries(regions)) {
    const expected = theme.contrast[name];
    const foregroundName = `${prefix}-foreground`;
    const resolvedMutedName = mutedName ?? `${prefix}-muted`;
    const foreground = hexToRgb(cssVariable(css, `codeskin-${foregroundName}`));
    const muted = hexToRgb(cssVariable(css, `codeskin-${resolvedMutedName}`));
    assert.equal(cssVariable(css, `codeskin-${foregroundName}`), expected.foreground);
    assert.equal(cssVariable(css, `codeskin-${resolvedMutedName}`), expected.muted);

    for (const [mode, overlayOpacity] of Object.entries(overlays)) {
      const composite = blendRgb(veil, sample, overlayOpacity);
      assert.ok(contrastRatio(foreground, composite) >= 4.5, `${mode} ${name} normal text must reach 4.5:1`);
      assert.ok(contrastRatio(muted, composite) >= 3, `${mode} ${name} muted text must reach 3:1`);
    }
  }
};

const welcomeWithoutBrittleMarkerUsesAmbientModeAndChatUsesFocusMode = () => {
  const welcome = createLiveContext();
  welcome.document.querySelector = (selector) => {
    if (selector === "main, [role='main']") return {};
    if (selector === "textarea, [contenteditable='true'], [role='textbox']") return {};
    return null;
  };
  const welcomeInstall = run(`(${installSource})`, welcome.context);
  assert.equal(welcomeInstall(theme).mode, "ambient");

  const chat = createLiveContext();
  chat.document.querySelector = (selector) => {
    if (selector === "main, [role='main']") return {};
    if (selector === "textarea, [contenteditable='true'], [role='textbox']") return {};
    if (selector === "[role='log'], [data-message-author-role]") return {};
    return null;
  };
  const chatInstall = run(`(${installSource})`, chat.context);
  assert.equal(chatInstall(theme).mode, "focus");

  const currentCodexChat = createLiveContext();
  currentCodexChat.document.querySelector = (selector) => {
    if (selector === "main, [role='main']") return {};
    if (selector === "[data-thread-find-target='conversation'], [data-user-message-bubble='true']") return {};
    return null;
  };
  const currentCodexChatInstall = run(`(${installSource})`, currentCodexChat.context);
  assert.equal(currentCodexChatInstall(theme).mode, "focus");
};

const observerTracksPortalAndModeAttributes = () => {
  const { context } = createLiveContext();
  const install = run(`(${installSource})`, context);
  assert.equal(install(theme).active, true);
  const observerOptions = context.window.__codeskinRuntime.observer.observed[0].options;
  assert.equal(observerOptions.childList, true);
  assert.equal(observerOptions.subtree, true);
  assert.equal(observerOptions.attributes, true);
  assert.ok(observerOptions.attributeFilter.includes("class"));
  assert.ok(observerOptions.attributeFilter.includes("aria-expanded"));
  assert.ok(observerOptions.attributeFilter.includes("data-state"));
};

pendingRuntimeWithNoRootIsRemovedBeforeDOMContentLoaded();
foreignPendingRuntimeIsNotRemoved();
installExposesObserverAndRestoreRemovesIt();
foreignObserverRemainsAndInstallFailsClosed();
repeatInstallReplacesOnlyItsOwnObserver();
restoreDoesNotDeleteOrDisconnectAForeignObserver();
verifyFailsClosedWhenObserverGlobalDoesNotMatchRuntime();
injectedSurfacesUseRegionalGlassWithoutGlassingTranscript();
elevatedMenusDialogsAndSettingsUseRegionalReadableGlass();
settingsCardsAndFogControlsUseRegionalAmbientGlass();
responsiveUtilitySidebarUsesContentGlassInFocusAndAmbientModes();
userMessageBubblesUseContentGlassWithoutMatchingHoverUtilities();
regionalGlassTextMeetsContrastAgainstItsSampledWallpaperArea();
currentTaskHeaderTitleUsesReadableSampledHeaderText();
openLocationButtonGroupUsesSampledHeaderGlassWithoutLocaleSelectors();
activitySummaryTrailingUsesReadableMutedColorWithoutGlobalSvgOverride();
directWallpaperTextUsesSampledRegionalContrastWithLightOverlays();
welcomeWithoutBrittleMarkerUsesAmbientModeAndChatUsesFocusMode();
observerTracksPortalAndModeAttributes();
console.log("injection runtime behavior tests passed");
