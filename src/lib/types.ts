// Mirrors the Rust models exposed over Tauri commands.

export interface LauncherSettings {
  min_memory_mb: number;
  max_memory_mb: number;
  java_path: string | null;
  concurrent_downloads: number;
}

export interface Instance {
  id: string;
  name: string;
  version_id: string;
  created_at: string;
  min_memory_mb: number | null;
  max_memory_mb: number | null;
  java_path: string | null;
  last_played_at: number | null;
  playtime_secs: number;
  dir: string;
  loader: string | null;
  loader_version: string | null;
  launch_version_id: string | null;
}

export type LoaderKind = "fabric" | "quilt" | "neoforge" | "forge";

export interface VersionEntry {
  id: string;
  type: string;
  url: string;
  time: string;
  releaseTime: string;
  sha1: string;
}

export interface JavaInfo {
  path: string;
  major: number;
}

export interface JavaStatus {
  required_major: number;
  found: JavaInfo | null;
  ok: boolean;
}

export interface InstallState {
  stage: string;
  completed: number;
  total: number;
  downloadedBytes: number;
  totalBytes: number;
}

export interface AccountView {
  id: string;
  name: string;
  active: boolean;
}

export interface DeviceCodeInfo {
  user_code: string;
  verification_uri: string;
  message: string;
}

export interface VersionMedia {
  image_url: string;
  short_text: string | null;
  accent: string | null;
  local: boolean;
}

export interface RunningInfo {
  running_id: string;
  instance_id: string;
  pid: number;
  started_at: number;
  state: string;
  exit_code: number | null;
}

export interface LogLine {
  stream: string;
  line: string;
}

export type View = "home" | "instances" | "accounts" | "settings" | "console";
