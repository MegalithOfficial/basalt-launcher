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
}

export type View = "home" | "instances" | "accounts" | "settings";
