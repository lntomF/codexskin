import { invoke } from "@tauri-apps/api/core";
import type { BackgroundLibrary, CodexStatus, VerifyResult } from "./types";

export const loadBackgroundLibrary = () =>
  invoke<BackgroundLibrary>("load_background_library");
export const inspectCodexStatus = () =>
  invoke<CodexStatus>("inspect_codex_status");
export const connectOrStartCodex = () =>
  invoke<CodexStatus>("connect_or_start_codex");
export const applyBackground = (backgroundId: string) =>
  invoke<VerifyResult>("apply_background", { backgroundId });
export const importBackground = (bytes: number[], displayName: string) =>
  invoke<BackgroundLibrary>("import_background", { bytes, displayName });
export const deleteBackground = (backgroundId: string) =>
  invoke<BackgroundLibrary>("delete_background", { backgroundId });
export const verifyInjection = () => invoke<VerifyResult>("verify_injection");
export const restoreOriginalAppearance = () =>
  invoke<VerifyResult>("restore_original_appearance");
