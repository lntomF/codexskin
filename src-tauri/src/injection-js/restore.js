(() => {
  const runtimeKey = "__codeskinRuntime";
  const observerKey = "__codeskinModeObserver";
  const runtimeOwner = "codeskin-runtime-v1";
  const runtimeVersion = 1;
  const styleId = "codeskin-runtime-style";
  const wallpaperId = "codeskin-wallpaper-layer";
  const ownerAttribute = "data-codeskin-theme-id";
  const modeAttribute = "data-codeskin-mode";
  const ownedAttribute = "data-codeskin-owned";
  const layerAttribute = "data-codeskin-layer";
  const runtimeAttribute = "data-codeskin-runtime";
  const root = document.documentElement;

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

  const isOwnedRuntime = (runtime) => Boolean(
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

  const isOwnedPendingRuntimeWithoutRoot = (runtime) => Boolean(
    runtime
      && typeof runtime === "object"
      && runtime.owner === runtimeOwner
      && runtime.version === runtimeVersion
      && runtime.root === null
      && runtime.style === null
      && runtime.wallpaper === null
      && runtime.observer === null
      && typeof runtime.pendingInstall === "function"
      && typeof runtime.modeUpdateQueued === "boolean"
  );

  const runtime = window[runtimeKey];
  if (isOwnedPendingRuntimeWithoutRoot(runtime)) {
    document.removeEventListener("DOMContentLoaded", runtime.pendingInstall);
    runtime.pendingInstall = null;
    if (window[observerKey] === runtime.observer) delete window[observerKey];
    if (window[runtimeKey] === runtime) delete window[runtimeKey];
    return { active: false };
  }

  if (!isOwnedRuntime(runtime)) return { active: false };

  if (typeof runtime.pendingInstall === "function") {
    document.removeEventListener("DOMContentLoaded", runtime.pendingInstall);
  }
  runtime.pendingInstall = null;

  if (runtime.observer) runtime.observer.disconnect();
  if (window[observerKey] === runtime.observer) delete window[observerKey];
  runtime.observer = null;
  runtime.modeUpdateQueued = false;

  if (isOwnedStyle(runtime.style)) runtime.style.remove();
  if (isOwnedWallpaper(runtime.wallpaper)) runtime.wallpaper.remove();

  root?.removeAttribute(ownerAttribute);
  root?.removeAttribute(modeAttribute);
  if (window[runtimeKey] === runtime) delete window[runtimeKey];
  return { active: false };
})()