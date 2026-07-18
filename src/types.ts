export type Background = {
  id: string;
  name: string;
  description: string;
  backgroundImage: string | null;
  sourceImage: string | null;
  previewDataUrl: string | null;
};

export type BackgroundLibrary = {
  version: number;
  selectedBackgroundId: string | null;
  backgrounds: Background[];
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
  wallpaperConfigured: boolean;
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
