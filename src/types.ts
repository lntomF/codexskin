export type ThemeColors = {
  accent: string;
  background: string;
  surface: string;
  foreground: string;
  muted: string;
};

export type ThemeSource = "builtin" | "wallpaper";

export type ThemeLayers = {
  ambientOverlayOpacity: number;
  focusOverlayOpacity: number;
  sidebarOpacity: number;
  cardOpacity: number;
};

export type Theme = {
  id: string;
  name: string;
  description: string;
  colors: ThemeColors;
  source: ThemeSource;
  layers: ThemeLayers;
  backgroundImage: string | null;
};

export type ThemeLibrary = {
  version: number;
  selectedThemeId: string | null;
  themes: Theme[];
};

export type CodexConnectionState =
  | "notRunning"
  | "runningWithoutDebugPort"
  | "debugPortDetected"
  | "starting"
  | "connecting"
  | "connected"
  | "reconnecting"
  | "error";

export type CodexStatus = {
  state: CodexConnectionState;
  port: number | null;
  executablePath: string | null;
  detail: string;
};

export type TargetVerification = {
  targetId: string;
  targetUrl: string;
  active: boolean;
  detail: string;
  wallpaperLayer: boolean;
  styleLayer: boolean;
  mode: string | null;
};

export type VerifyResult = {
  themeId: string | null;
  active: boolean;
  targets: TargetVerification[];
};

export type CommandError = {
  code: string;
  message: string;
};

