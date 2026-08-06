
export interface LauncherSettings {
  min_memory_mb: number;
  max_memory_mb: number;
  java_path: string | null;
  concurrent_downloads: number;
  curseforge_api_key: string | null;
  log_level: LogLevel;
  jvm_args: string;
  game_args: string;
  window_width: number;
  window_height: number;
  fullscreen: boolean;
  ignore_java_checks: boolean;
  env_vars: EnvVar[];
  proxy_mode: ProxyMode;
  proxy_host: string;
  proxy_port: number;
  proxy_username: string;
  proxy_password: string;
  request_timeout_secs: number;
  max_retries: number;
  allow_insecure_tls: boolean;
  onboarded: boolean;
  accent_mode: AccentMode;
  accent_color: string;
  ok_color: string;
  warn_color: string;
  danger_color: string;
  show_suggestions: boolean;
  pack_content_updates: boolean;
  minimize_on_launch: boolean;
  auto_update_checks: boolean;
  wrapper_command: string;
  pre_launch_command: string;
  post_exit_command: string;
  discord_rpc: boolean;
  discord_rpc_show_version: boolean;
  discord_rpc_show_streak: boolean;
  discord_rpc_show_logo: boolean;
  discord_app_id: string;
}

export type AccentMode = "banner" | "custom" | "default";

export type ProxyMode = "system" | "none" | "http" | "socks5";

export interface NetworkProbe {
  ok: boolean;
  status: number | null;
  millis: number;
  via_proxy: boolean;
  error: string | null;
}

export interface EnvVar {
  key: string;
  value: string;
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
  logo: string | null;
  loader: string | null;
  loader_version: string | null;
  launch_version_id: string | null;
  pack_provider: string | null;
  pack_project_id: string | null;
  pack_version_id: string | null;
  jvm_args: string | null;
  jvm_args_mode: string | null;
  env_vars: string | null;
  env_vars_mode: string | null;
  banner_id: string | null;
  notes: string | null;
  wrapper_command: string | null;
  pre_launch_command: string | null;
  post_exit_command: string | null;
}

export interface InstanceGroup {
  id: string;
  name: string;
  sort_order: number;
}

export interface InstancePlacement {
  instance_id: string;
  group_id: string | null;
  sort_order: number;
}

export interface InstanceOrganization {
  groups: InstanceGroup[];
  placements: InstancePlacement[];
}

export type LoaderKind = "fabric" | "quilt" | "neoforge" | "forge";

export type ContentKind =
  | "mods"
  | "resourcepacks"
  | "shaderpacks"
  | "schematics"
  | "datapacks"
  | "modpacks";

export type ContentOrigin = "user" | "dependency" | "pack" | "manual";

export interface ManualDownload {
  project_id: string;
  file_id: string;
  file_name: string;
  download_page_url: string;
  sha1: string | null;
  size: number | null;
  instance_path: string;
  pack_archive: boolean;
}

export interface ManualDownloadSource {
  project_id: string;
  file_id: string;
  path: string;
}

export interface ModpackInstallPlan {
  manual_downloads: ManualDownload[];
}

export interface ModpackUpgrade {
  current_version_id: string;
  target_version_id: string;
  target_name: string;
  version_number: string;
  channel: string;
  date: string;
  game_version: string;
  loader: string | null;
  loader_version: string | null;
}

export interface ModpackUpgradeChanges {
  added: string[];
  removed: string[];
  changed: string[];
  preserved: string[];
  unchanged: number;
}

export interface ModpackUpgradePlan {
  update: ModpackUpgrade;
  manual_downloads: ManualDownload[];
  changes: ModpackUpgradeChanges | null;
}

export interface ContentFile {
  file_name: string;
  sha1: string | null;
  sha512: string | null;
  murmur2: number | null;
  provider: SearchProvider | null;
  project_id: string | null;
  version_id: string | null;
  title: string | null;
  icon_url: string | null;
  mod_id: string | null;
  mod_version: string | null;
  dependencies: string | null;
  origin: ContentOrigin;
  pack_version_id: string | null;
  installed_at: number;
}

export interface ContentUpdate {
  kind: string;
  file_name: string;
  latest_version_id: string;
  latest_name: string;
  latest_file_name: string;
}

export interface ContentItem {
  file_name: string;
  size: number;
  enabled: boolean;
  source: ContentFile | null;
  update: ContentUpdate | null;
}

export type SearchProvider = "modrinth" | "curseforge";

export type SortOrder = "relevance" | "downloads" | "follows" | "newest" | "updated";

export type Environment = "client" | "server";

export interface SearchQuery {
  query: string;
  game_versions: string[];
  loaders: string[];
  categories: string[];
  environment: Environment | null;
  open_source_only: boolean;
  sort: SortOrder;
  offset: number;
  limit: number;
}

export interface ProjectSummary {
  id: string;
  slug: string | null;
  title: string;
  description: string;
  icon_url: string | null;
  downloads: number;
  follows: number;
  author: string;
  categories: string[];
  game_versions: string[];
  loaders: string[];
  updated: string | null;
  color: number | null;
}

export interface SearchPage {
  hits: ProjectSummary[];
  total: number;
  offset: number;
  limit: number;
}

export interface FilterOption {
  id: string;
  name: string;
  group: string;
}

export interface FilterTaxonomy {
  categories: FilterOption[];
  loaders: FilterOption[];
  game_versions: string[];
}

export interface VersionFile {
  url: string | null;
  file_name: string;
  sha1: string | null;
  sha512: string | null;
  size: number | null;
  primary: boolean;
}

export interface ProjectVersion {
  id: string;
  project_id: string;
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
  files: VersionFile[];
}

export interface VersionDependency {
  project_id: string;
  version_id: string | null;
  dependency_type: string;
}

export interface PlannedFile {
  project_id: string;
  version_id: string;
  title: string;
  icon_url: string | null;
  file_name: string;
  version_name: string;
  url: string;
  sha1: string | null;
  sha512: string | null;
  size: number | null;
  is_dependency: boolean;
  replaces: string | null;
  dependencies: VersionDependency[];
}

export interface SkippedProject {
  project_id: string;
  title: string;
  icon_url: string | null;
  reason: string;
}

export interface Conflict {
  project_id: string;
  title: string;
  file_name: string | null;
  reason: string;
}

export interface InstallPlan {
  primary: PlannedFile | null;
  dependencies: PlannedFile[];
  already_present: ProjectSummary[];
  skipped: SkippedProject[];
  conflicts: Conflict[];
  total_bytes: number;
}

export type LauncherKind = "atlauncher" | "prism" | "modrinth";

export interface LauncherSource {
  kind: LauncherKind;
  label: string;
  root: string;
  instance_count: number;
}

export interface MigrationCandidate {
  id: string;
  name: string;
  version_id: string;
  loader: string | null;
  loader_version: string | null;
  icon_data_url: string | null;
  pack: string | null;
  mod_count: number;
  file_count: number;
  total_bytes: number;
  last_played_ms: number | null;
  warnings: string[];
  importable: boolean;
  imported: boolean;
}

export interface MigrationScan {
  kind: LauncherKind;
  root: string;
  candidates: MigrationCandidate[];
}

export interface MigrationOutcome {
  imported: string[];
  failed: Array<[string, string]>;
}

export type PackFormat = "mrpack" | "curseforge";

export interface PackPreview {
  format: PackFormat;
  name: string;
  version: string | null;
  author: string | null;
  game_version: string;
  loader: string | null;
  loader_version: string | null;
  declared_files: number;
  override_files: number;
  override_bytes: number;
  warnings: string[];
  importable: boolean;
}

export interface PackExport {
  path: string;
  format: PackFormat;
  linked: number;
  bundled: number;
  bytes: number;
}

export type TaskKind =
  | "game_install"
  | "java_install"
  | "loader_install"
  | "modpack_install"
  | "modpack_upgrade"
  | "content_install"
  | "content_update"
  | "world_import"
  | "instance_import"
  | "app_update"
  | "instance_repair"
  | "instance_duplicate"
  | "snapshot_create"
  | "snapshot_restore"
  | "storage_scan"
  | "datapack_install";

export interface RepairReport {
  checked_content: number;
  repaired_content: number;
  unresolved: string[];
}

export interface SnapshotSummary {
  id: string;
  name: string;
  kind: "manual" | "automatic";
  created_at: number;
  file_count: number;
  size_bytes: number;
  stored_size_bytes: number;
  new_size_bytes: number | null;
  excluded: string[];
}

export type WorldStatus = "ok" | "recovered" | "damaged";

export interface WorldSummary {
  folder_name: string;
  name: string;
  last_played_ms: number | null;
  version_name: string | null;
  data_version: number | null;
  game_mode: string;
  hardcore: boolean;
  difficulty: number | null;
  icon_data_url: string | null;
  status: WorldStatus;
  error: string | null;
}

export interface WorldImportCandidate {
  id: string;
  archive_root: string;
  name: string;
  last_played_ms: number | null;
  version_name: string | null;
  data_version: number | null;
  game_mode: string;
  hardcore: boolean;
  status: WorldStatus;
  error: string | null;
  file_count: number;
  total_bytes: number;
}

export interface WorldImportInspection {
  source_kind: "directory" | "zip";
  candidates: WorldImportCandidate[];
}

export type TaskState = "running" | "succeeded" | "failed" | "cancelled";

export interface Task {
  id: string;
  kind: TaskKind;
  title: string;
  subtitle: string | null;
  icon_url: string | null;
  instance_id: string | null;
  project_id: string | null;
  state: TaskState;
  stage: string;
  completed: number;
  total: number;
  downloaded_bytes: number;
  total_bytes: number;
  error: string | null;
  retries: number;
  retry_note: string | null;
  started_at: number;
  finished_at: number | null;
}

export interface PendingOperation {
  id: string;
  kind: TaskKind;
  instance_id: string | null;
  title: string;
  payload: string | null;
  started_at: number;
}

export interface ContentProgress {
  completed: number;
  total: number;
  current: string;
}

export interface OrphanFile {
  file_name: string;
  title: string;
  icon_url: string | null;
}

export interface RemovalPlan {
  dependents: string[];
  from_pack: boolean;
  orphans: OrphanFile[];
}

export interface InstalledItem {
  file_name: string;
  title: string;
  icon_url: string | null;
  is_dependency: boolean;
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

export interface GalleryImage {
  url: string;
  raw_url: string | null;
  title: string | null;
  description: string | null;
  featured: boolean;
}

export interface ProjectDetails {
  id: string;
  slug: string | null;
  title: string;
  description: string;
  body: string;
  body_format: "markdown" | "html";
  icon_url: string | null;
  downloads: number;
  follows: number;
  author: string;
  gallery: GalleryImage[];
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
  color: number | null;
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

export interface AppInfo {
  version: string;
  build_channel: "dev" | "release";
  data_dir: string;
  default_jvm_args: string;
  jvm_placeholders: string[];
  arch: string;
  install_source: InstallSource;
  bundled_curseforge_key: boolean;
  bundled_discord_app_id: boolean;
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
  kind: BannerKind;
}

export type BannerKind = "image" | "video";

export interface BannerEntry {
  id: string;
  path: string;
  kind: BannerKind;
  original_name: string | null;
  width: number | null;
  height: number | null;
  bytes: number;
  accent: string | null;
  added_at: number;
  in_use_by: string[];
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

export type LogLevel = "error" | "warn" | "info" | "debug" | "trace";

export type LogSource = "backend" | "frontend";

export interface LogRecord {
  seq: number;
  ts: number;
  level: LogLevel;
  source: LogSource;
  target: string;
  span: string | null;
  message: string;
  fields: Record<string, string>;
}

export type LogsTab = "launcher" | "game" | "files";

export interface Screenshot {
  name: string;
  path: string;
  size_bytes: number;
  modified_ms: number;
  thumbnail: string | null;
}

export interface Thumbnail {
  name: string;
  path: string | null;
}

export interface InstanceLogFile {
  name: string;
  size_bytes: number;
  modified_ms: number;
  compressed: boolean;
  crash: boolean;
}

export interface LogHit {
  number: number;
  line: string;
  ranges: Array<[number, number]>;
  level: "error" | "warn" | "info" | "debug";
}

export interface LogSearch {
  hits: LogHit[];
  total_lines: number;
  matched_lines: number;
  truncated: boolean;
}

export interface LogConfig {
  level: LogLevel;
  directory: string;
  file: string;
  env_override: string | null;
  levels: LogLevel[];
}

export type View =
  | "home"
  | "instances"
  | "accounts"
  | "settings"
  | "instance"
  | "discover"
  | "project"
  | "stats"
  | "logs";

export interface PlaySession {
  id: number;
  instance_id: string;
  instance_name: string;
  started_at: number;
  ended_at: number;
  played_secs: number;
  crashed: boolean;
  version_id: string | null;
  loader: string | null;
}

export interface DayBucket {
  date: string;
  secs: number;
  sessions: number;
}

export interface InstancePlayStat {
  instance_id: string;
  name: string;
  secs: number;
  sessions: number;
  crashes: number;
  last_played_at: number | null;
  lifetime_secs: number;
  deleted: boolean;
}

export interface LoaderPlayStat {
  loader: string;
  secs: number;
  sessions: number;
}

export interface PlayStats {
  lifetime_secs: number;
  tracked_since: number | null;
  window_days: number | null;
  window_secs: number;
  session_count: number;
  crash_count: number;
  longest_session_secs: number;
  average_session_secs: number;
  active_days: number;
  current_streak_days: number;
  longest_streak_days: number;
  busiest_day: DayBucket | null;
  daily: DayBucket[];
  hourly: number[];
  weekday: number[];
  instances: InstancePlayStat[];
  loaders: LoaderPlayStat[];
  recent: PlaySession[];
  recent_total: number;
  recent_page: number | null;
}

export interface UpdateInfo {
  current: string;
  latest: string | null;
  notes_url: string | null;
  published_at: string | null;
  update_available: boolean;
  install_source: InstallSource;
}

export type UpdatePolicy = "self_managed" | "package_managed" | "manual";

export interface InstallSource {
  id: string;
  label: string;
  policy: UpdatePolicy;
  update_hint: string;
}

export type AppUpdatePhase = "idle" | "available" | "downloading" | "ready";

export interface AppUpdateStatus {
  phase: AppUpdatePhase;
  info: UpdateInfo | null;
  dismissed: boolean;
  last_checked_at: number | null;
}

export interface AboutLinks {
  repository: string;
  issues: string;
  releases: string;
}

export type SkinVariant = "classic" | "slim";

export interface SkinEntry {
  id: string;
  name: string;
  variant: SkinVariant;
  source: string | null;
  data_url: string;
}

export interface CapeEntry {
  id: string;
  alias: string;
  url: string;
  active: boolean;
}

export interface Appearance {
  uuid: string;
  name: string;
  skin_url: string | null;
  variant: SkinVariant;
  capes: CapeEntry[];
  active_cape_id: string | null;
  library_id: string | null;
}

export interface SystemStats {
  os: string;
  kernel: string | null;
  cpu: string;
  cores: number;
  total_memory_mb: number;
  available_memory_mb: number;
  data_dir_free_mb: number | null;
  data_dir_total_mb: number | null;
}

export type DataRoot =
  | "instances"
  | "versions"
  | "libraries"
  | "assets"
  | "natives"
  | "runtimes"
  | "snapshots"
  | "cache";

export interface DiskInfo {
  mount_point: string;
  name: string;
  free_mb: number;
  total_mb: number;
  removable: boolean;
}

export interface DataLocation {
  slot: DataRoot;
  label: string;
  summary: string;
  path: string;
  default_path: string;
  custom: boolean;
  exists: boolean;
  disk: DiskInfo | null;
}

export interface LocationCandidate {
  path: string;
  usable: boolean;
  problem: string | null;
  occupied: boolean;
  disk: DiskInfo | null;
}

export interface LaunchPreview {
  java: string;
  pinned: boolean;
  jvm: string[];
  game: string[];
}

export interface SystemUsage {
  total_memory_mb: number;
  available_memory_mb: number;
  data_dir_free_mb: number | null;
  data_dir_total_mb: number | null;
}

export type DiagnosisFix =
  | "none"
  | "open_mods_folder"
  | { install_java: { major: number } }
  | { find_content: { query: string } }
  | { raise_memory: { megabytes: number } };

export interface Diagnosis {
  id: string;
  title: string;
  detail: string;
  subjects: string[];
  fix: DiagnosisFix;
}

export interface StorageEntry {
  id: string;
  label: string;
  bytes: number;
  path: string | null;
  children: StorageEntry[];
}

export type StorageTier = "cache" | "shared" | "spare";

export interface Reclaimable {
  id: string;
  label: string;
  detail: string;
  bytes: number;
  count: number;
  tier: StorageTier;
  items: string[];
}

export interface StorageReport {
  scanned_at: number;
  root: string;
  total_bytes: number;
  free_bytes: number | null;
  disk_total_bytes: number | null;
  buckets: StorageEntry[];
  reclaimable: Reclaimable[];
  unresolved: string | null;
  shared_dedupe: boolean;
}

export interface ReclaimOutcome {
  freed_bytes: number;
  cleared: string[];
  failures: string[];
}

export interface PathKind {
  path: string;
  directory: boolean;
  usable: boolean;
}

export type PackCompatibility =
  | { state: "fits" }
  | { state: "unknown" }
  | { state: "mismatch"; needs: number; has: number };

export interface Datapack {
  file_name: string;
  enabled: boolean;
  off_in_game: boolean;
  directory: boolean;
  size: number;
  title: string | null;
  min_format: number | null;
  max_format: number | null;
  compatibility: PackCompatibility;
  provider: string | null;
  project_id: string | null;
  version_id: string | null;
  icon_url: string | null;
  latest_version_id: string | null;
  latest_file_name: string | null;
}

export interface WorldPacks {
  world: string;
  display_name: string;
  loose: boolean;
  packs: Datapack[];
}
