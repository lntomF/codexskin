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
    background: "#1A1B26",
    surface: "#24283B",
    foreground: "#C0CAF5",
    muted: "#565F89"
  },
  layers: {}
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

pendingRuntimeWithNoRootIsRemovedBeforeDOMContentLoaded();
foreignPendingRuntimeIsNotRemoved();
installExposesObserverAndRestoreRemovesIt();
foreignObserverRemainsAndInstallFailsClosed();
repeatInstallReplacesOnlyItsOwnObserver();
restoreDoesNotDeleteOrDisconnectAForeignObserver();
verifyFailsClosedWhenObserverGlobalDoesNotMatchRuntime();
console.log("injection runtime behavior tests passed");