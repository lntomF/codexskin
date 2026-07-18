(() => {
  const runtimeKey = "__codeskinRuntime";
  const observerKey = "__codeskinModeObserver";
  const runtimeOwner = "codeskin-runtime-v1";
  const runtimeVersion = 1;
  const styleId = "codeskin-runtime-style";
  const wallpaperId = "codeskin-wallpaper-layer";
  const ownerAttribute = "data-codeskin-theme-id";
  const modeAttribute = "data-codeskin-mode";
  const colorSchemeAttribute = "data-codeskin-color-scheme";
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
      && isOwnedStyle(runtime.style)
      && isOwnedWallpaper(runtime.wallpaper)
      && (runtime.observer === null || typeof runtime.observer.disconnect === "function")
      && (runtime.pendingInstall === null || typeof runtime.pendingInstall === "function")
      && typeof runtime.modeUpdateQueued === "boolean"
  );

  const runtime = window[runtimeKey];
  const runtimeIsOwned = isOwnedRuntime(runtime);
  const observerGlobalMatchesRuntime = Boolean(
    runtimeIsOwned
      && runtime.observer !== null
      && window[observerKey] === runtime.observer
  );
  const safe = Boolean(runtimeIsOwned && observerGlobalMatchesRuntime);
  const wallpaperLayer = runtimeIsOwned && runtime.wallpaper.isConnected;
  const styleLayer = runtimeIsOwned && runtime.style.isConnected;
  const wallpaperImage = runtimeIsOwned ? runtime.wallpaper.style.backgroundImage : "";
  const wallpaperConfigured = typeof wallpaperImage === "string"
    && /^url\("data:image\/jpeg;base64,[A-Za-z0-9+/]+={0,2}"\)$/.test(wallpaperImage);
  const themeId = runtimeIsOwned ? root?.getAttribute(ownerAttribute) || null : null;
  const mode = runtimeIsOwned ? root?.getAttribute(modeAttribute) || null : null;
  const accent = runtimeIsOwned && root
    ? getComputedStyle(root).getPropertyValue("--codeskin-accent").trim()
    : "";
  const secondary = runtimeIsOwned && root
    ? getComputedStyle(root).getPropertyValue("--codeskin-secondary").trim()
    : "";
  const colorScheme = runtimeIsOwned ? root?.getAttribute(colorSchemeAttribute) || null : null;
  const validThemeId = typeof themeId === "string" && /^[A-Za-z0-9]+(?:[A-Za-z0-9-]*[A-Za-z0-9])?$/.test(themeId);
  const validMode = mode === "ambient" || mode === "focus";
  const validSecondary = /^#[0-9A-Fa-f]{6}$/.test(secondary);
  const validColorScheme = colorScheme === "light" || colorScheme === "dark";

  return {
    active: Boolean(safe && wallpaperLayer && styleLayer && wallpaperConfigured && validThemeId && validMode && validSecondary && validColorScheme),
    safe,
    themeId,
    accent,
    secondary,
    colorScheme,
    wallpaperLayer: Boolean(wallpaperLayer),
    wallpaperConfigured: Boolean(wallpaperConfigured),
    styleLayer: Boolean(styleLayer),
    mode
  };
})()