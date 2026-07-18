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
      panelOpacity: 0.18, blurPx: 8, textShadow: "0 1px 2px rgba(255,255,255,0.4)"
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

const injectedSurfaceKeepsWallpaperSharpAndOnlyComposerGlass = () => {
  const { document, context } = createLiveContext();
  const install = run(`(${installSource})`, context);

  assert.equal(install(theme).active, true);
  const css = document.getElementById("codeskin-runtime-style").textContent;
  const mainSurfaceRule = cssRule(css, ":root[data-codeskin-theme-id] .main-surface");
  const headerRule = cssRule(css, ":root[data-codeskin-theme-id] .app-header-tint");
  const sidebarRule = cssRule(css, ":root[data-codeskin-theme-id] .app-shell-left-panel");
  const composerRule = cssRule(css, ":root[data-codeskin-theme-id] .composer-surface-chrome");

  assert.match(css, /--codeskin-secondary: #BB9AF7;/);
  assert.match(mainSurfaceRule, /background: transparent !important;/);
  assert.match(mainSurfaceRule, /backdrop-filter: none !important;/);
  assert.doesNotMatch(mainSurfaceRule, /backdrop-filter: blur/);
  assert.match(css, /--codeskin-sidebar-foreground: #F4F7FF;/);
  assert.match(css, /--codeskin-content-foreground: #172033;/);
  assert.match(css, /--codeskin-header-foreground: #263744;/);
  assert.match(css, /--codeskin-header-muted-foreground: #52616D;/);
  assert.match(css, /--codeskin-info-foreground: #F4F7FF;/);
  assert.match(css, /text-shadow: var\(--codeskin-sidebar-text-shadow\) !important;/);
  assert.match(css, /text-shadow: var\(--codeskin-content-text-shadow\) !important;/);
  assert.match(css, /text-shadow: var\(--codeskin-info-text-shadow\) !important;/);

  assert.match(composerRule, /backdrop-filter: blur\(var\(--codeskin-composer-blur\)\) saturate\(112%\);/);
  assert.match(composerRule, /background-color: color-mix/);
  assert.match(composerRule, /box-shadow:/);
  for (const rule of [headerRule, sidebarRule]) {
    assert.match(rule, /background: transparent !important;/);
    assert.doesNotMatch(rule, /(?:background-color:|border(?:-color)?:|backdrop-filter: blur|box-shadow:)/);
  }
  assert.match(
    css,
    /\[class\*="_markdownContent_"\][\s\S]*?background: transparent !important;/,
    "chat and code wrappers must be explicitly transparent rather than inheriting an opaque Codex surface"
  );
  assert.match(
    css,
    /\[class\*="bg-token-foreground\/5"\][\s\S]*?background: transparent !important;/,
    "environment information must be explicitly transparent rather than a glass card"
  );
  assert.doesNotMatch(css, /--codeskin-(?:sidebar|content|info)-panel-(?:color|opacity)/);
  assert.doesNotMatch(css, /--codeskin-(?:sidebar|content|info)-blur/);
  assert.doesNotMatch(css, /\[role="dialog"\]|\[role="listbox"\]/);
  assert.doesNotMatch(css, /:root\[data-codeskin-theme-id\] button,/);
};

const textRegionsUsePerImageContrastWithoutGlassPanels = () => {
  const { document, context } = createLiveContext();
  const install = run(`(${installSource})`, context);

  assert.equal(install(theme).active, true);
  const css = document.getElementById("codeskin-runtime-style").textContent;

  assert.match(css, /\.app-shell-left-panel :is\([\s\S]*?color: var\(--codeskin-sidebar-foreground\) !important;/);
  assert.match(css, /\.main-surface :is\([\s\S]*?color: var\(--codeskin-content-foreground\) !important;/);
  assert.match(css, /\[class\*="bg-token-foreground\/5"\] :is\([\s\S]*?color: var\(--codeskin-info-foreground\) !important;/);
  assert.doesNotMatch(css, /--codeskin-(?:sidebar|content|info)-panel-(?:color|opacity)/);
  assert.doesNotMatch(css, /--codeskin-(?:sidebar|content|info)-blur/);
};

const environmentInfoAndTopMenusUseContrastWithoutChangingTheirSurfaces = () => {
  const { document, context } = createLiveContext();
  const install = run(`(${installSource})`, context);

  assert.equal(install(theme).active, true);
  const css = document.getElementById("codeskin-runtime-style").textContent;
  const environmentRule = cssRule(
    css,
    ':root[data-codeskin-theme-id] [class*="bg-token-dropdown-background"]:has([class~="group/summary-panel-item"])'
  );
  const toolbarRule = cssRule(
    css,
    ':root[data-codeskin-theme-id] .app-header-tint[class*="application-menu-top-bar"]\nbutton.no-drag[aria-haspopup="menu"]'
  );
  const menuRule = cssRule(css, ':root[data-codeskin-theme-id] [role="menu"]');

  // The user-facing "审阅 / 终端 / 浏览器 / 文件 / 侧边任务" entries live in
  // .main-surface and are covered by its native token-text rule, rather than by
  // the top-level trigger selector above.
  assert.match(
    css,
    /:root\[data-codeskin-theme-id\] \.main-surface :is\([\s\S]*?\[class\*="text-token-text-primary"\][\s\S]*?color: var\(--codeskin-content-foreground\) !important;/
  );

  assert.match(environmentRule, /color: var\(--codeskin-info-foreground\) !important;/);
  assert.match(environmentRule, /text-shadow: var\(--codeskin-info-text-shadow\) !important;/);
  assert.doesNotMatch(environmentRule, /(?:background(?:-color)?:|border(?:-color)?:|backdrop-filter|box-shadow:)/);

  assert.match(toolbarRule, /color: var\(--codeskin-header-foreground\) !important;/);
  assert.match(toolbarRule, /text-shadow: var\(--codeskin-header-text-shadow\) !important;/);
  assert.doesNotMatch(toolbarRule, /--codeskin-content-foreground/);
  assert.doesNotMatch(toolbarRule, /(?:background(?:-color)?:|border(?:-color)?:|backdrop-filter|box-shadow:)/);

  assert.match(menuRule, /color: var\(--codeskin-content-foreground\) !important;/);
  assert.match(menuRule, /text-shadow: var\(--codeskin-content-text-shadow\) !important;/);
  assert.doesNotMatch(menuRule, /(?:background(?:-color)?:|border(?:-color)?:|backdrop-filter|box-shadow:)/);

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
injectedSurfaceKeepsWallpaperSharpAndOnlyComposerGlass();
textRegionsUsePerImageContrastWithoutGlassPanels();
environmentInfoAndTopMenusUseContrastWithoutChangingTheirSurfaces();
console.log("injection runtime behavior tests passed");
