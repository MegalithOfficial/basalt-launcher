// Mirrors the Rust models exposed over Tauri commands.

export interface LauncherSettings {
  min_memory_mb: number;
  max_memory_mb: number;
  java_path: string | null;
  concurrent_downloads: number;
  curseforge_api_key: string | null;
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

export type ContentKind = "mods" | "resourcepacks" | "shaderpacks" | "schematics";

export interface ContentSource {
  provider: SearchProvider;
  project_id: string;
  version_id: string | null;
  title: string | null;
  icon_url: string | null;
}

export interface ContentSourceEntry extends ContentSource {
  file_name: string;
}

export interface ContentItem {
  file_name: string;
  size: number;
  enabled: boolean;
  source: ContentSource | null;
}

export type SearchProvider = "modrinth" | "curseforge";

export interface SearchResult {
  id: string;
  title: string;
  description: string;
  icon_url: string | null;
  downloads: number;
  author: string;
}

export interface ProjectVersion {
  id: string;
  name: string;
  version_number: string;
  channel: string;
  date: string;
  downloads: number;
  file_name: string;
  size: number | null;
  game_versions: string[];
  loaders: string[];
  compatible: boolean;
  changelog: string | null;
  dependencies: VersionDependency[];
}

export interface VersionDependency {
  project_id: string;
  dependency_type: string;
}

export interface InstalledFile {
  version_id: string | null;
  file_name: string;
}

export interface Changelog {
  body: string;
  format: "markdown" | "html";
}

export interface ProjectLink {
  label: string;
  url: string;
}

export interface ProjectDetails {
  id: string;
  title: string;
  description: string;
  body: string;
  body_format: "markdown" | "html";
  icon_url: string | null;
  downloads: number;
  author: string;
  gallery: string[];
  game_versions: string[];
  loaders: string[];
  client_side: string | null;
  server_side: string | null;
  categories: string[];
  license: string | null;
  links: ProjectLink[];
  published: string | null;
  updated: string | null;
  website_url: string | null;
}

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

export type View =
  | "home"
  | "instances"
  | "accounts"
  | "settings"
  | "console"
  | "instance"
  | "search"
  | "project";
