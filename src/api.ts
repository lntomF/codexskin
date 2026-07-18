import { invoke } from "@tauri-apps/api/core";
import type { CodexStatus, ThemeLibrary, VerifyResult } from "./types";

export const loadThemeLibrary = () => invoke<ThemeLibrary>("load_theme_library");
export const inspectCodexStatus = () => invoke<CodexStatus>("inspect_codex_status");
export const connectOrStartCodex = () => invoke<CodexStatus>("connect_or_start_codex");
export const applyTheme = (themeId: string) =>
  invoke<VerifyResult>("apply_theme", { themeId });
export const verifyTheme = () => invoke<VerifyResult>("verify_theme");
export const restoreTheme = () => invoke<VerifyResult>("restore_theme");
export const importWallpaperTheme = (bytes: number[], displayName: string) =>
  invoke<ThemeLibrary>("import_wallpaper_theme", { bytes, displayName });
export const renameTheme = (themeId: string, name: string) =>
  invoke<ThemeLibrary>("rename_theme", { themeId, name });
