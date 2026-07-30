import { invoke } from "@tauri-apps/api/core";

import { log } from "./log";
import type {
  AboutLinks,
  AccountView,
  Appearance,
  AppInfo,
  Changelog,
  ContentItem,
  ContentUpdate,
  DeviceCodeInfo,
  FilterTaxonomy,
  InstallPlan,
  InstalledFile,
  InstalledItem,
  Instance,
  JavaInfo,
  JavaStatus,
  LauncherSource,
  LaunchPreview,
  MigrationOutcome,
  MigrationScan,
  LauncherSettings,
  InstanceLogFile,
  LogConfig,
  LogLine,
  LogSearch,
  LogRecord,
  ProjectDetails,
  RemovalPlan,
  ProjectSummary,
  ProjectVersion,
  RunningInfo,
  SearchPage,
  SearchQuery,
  PendingOperation,
  SkinEntry,
  SystemStats,
  SystemUsage,
  Task,
  UpdateInfo,
  VersionEntry,
  VersionMedia,
  WorldImportInspection,
  WorldSummary,
} from "./types";

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const started = performance.now();
  try {
    const result = await invoke<T>(command, args);
    log.debug("ipc", `${command} ok`, { ms: Math.round(performance.now() - started) });
    return result;
  } catch (error) {
    log.error("ipc", `${command} failed: ${String(error)}`, {
      ms: Math.round(performance.now() - started),
    });
    throw error;
  }
}

export const api = {
  getSettings: () => call<LauncherSettings>("get_settings"),
  getAppInfo: () => call<AppInfo>("get_app_info"),
  listJavas: () => call<JavaInfo[]>("list_javas"),
  updateSettings: (settings: LauncherSettings) =>
    call<void>("update_settings", { settings }),
  listInstances: () => call<Instance[]>("list_instances"),
  createInstance: (
    name: string,
    versionId: string,
    loader: string | null = null,
    loaderVersion: string | null = null,
  ) =>
    call<Instance>("create_instance", { name, versionId, loader, loaderVersion }),
  listLoaderVersions: (loader: string, gameVersion: string) =>
    call<string[]>("list_loader_versions", { loader, gameVersion }),
  listInstanceContent: (instanceId: string, kind: string, reconcile = false) =>
    call<ContentItem[]>("list_instance_content", { instanceId, kind, reconcile }),
  listInstanceWorlds: (instanceId: string) =>
    call<WorldSummary[]>("list_instance_worlds", { instanceId }),
  inspectWorldSource: (sourcePath: string) =>
    call<WorldImportInspection>("inspect_world_source", { sourcePath }),
  deleteInstanceWorld: (instanceId: string, folderName: string) =>
    call<void>("delete_instance_world", { instanceId, folderName }),
  importWorlds: (
    instanceId: string,
    sourcePath: string,
    candidateIds: string[],
  ) =>
    call<number>("import_worlds", { instanceId, sourcePath, candidateIds }),
  listInstanceContentBundle: (instanceId: string, kinds: string[], reconcile = false) =>
    call<Record<string, ContentItem[]>>("list_instance_content_bundle", {
      instanceId,
      kinds,
      reconcile,
    }),
  toggleInstanceContent: (instanceId: string, kind: string, fileName: string) =>
    call<boolean>("toggle_instance_content", { instanceId, kind, fileName }),
  deleteInstanceContent: (instanceId: string, kind: string, fileName: string) =>
    call<void>("delete_instance_content", { instanceId, kind, fileName }),
  addInstanceContent: (instanceId: string, kind: string, sources: string[]) =>
    call<number>("add_instance_content", { instanceId, kind, sources }),
  searchContent: (provider: string, kind: string, query: SearchQuery) =>
    call<SearchPage>("search_content", { provider, kind, query }),
  getFilterTaxonomy: (provider: string, kind: string, includeSnapshots = false) =>
    call<FilterTaxonomy>("get_filter_taxonomy", { provider, kind, includeSnapshots }),
  getVersionChangelog: (provider: string, projectId: string, versionId: string) =>
    call<Changelog>("get_version_changelog", { provider, projectId, versionId }),
  resolveProjects: (provider: string, ids: string[]) =>
    call<ProjectSummary[]>("resolve_projects", { provider, ids }),
  getInstalledProjectFile: (instanceId: string, kind: string, projectId: string) =>
    call<InstalledFile | null>("get_installed_project_file", {
      instanceId,
      kind,
      projectId,
    }),
  getProjectDetails: (provider: string, projectId: string) =>
    call<ProjectDetails>("get_project_details", { provider, projectId }),
  listProjectVersions: (
    provider: string,
    projectId: string,
    kind: string,
    gameVersion: string,
    loader: string | null,
  ) =>
    call<ProjectVersion[]>("list_project_versions", {
      provider,
      projectId,
      kind,
      gameVersion,
      loader,
    }),
  planContentInstall: (
    provider: string,
    projectId: string,
    instanceId: string,
    kind: string,
    gameVersion: string,
    loader: string | null,
    versionId: string | null = null,
  ) =>
    call<InstallPlan>("plan_content_install", {
      provider,
      projectId,
      instanceId,
      kind,
      gameVersion,
      loader,
      versionId,
    }),
  installContent: (
    provider: string,
    projectId: string,
    instanceId: string,
    kind: string,
    gameVersion: string,
    loader: string | null,
    versionId: string | null = null,
    withDependencies = true,
  ) =>
    call<InstalledItem[]>("install_content", {
      provider,
      projectId,
      instanceId,
      kind,
      gameVersion,
      loader,
      versionId,
      withDependencies,
    }),
  installModpack: (provider: string, projectId: string, versionId: string) =>
    call<Instance>("install_modpack", { provider, projectId, versionId }),
  checkContentUpdates: (instanceId: string, force = false) =>
    call<ContentUpdate[]>("check_content_updates", { instanceId, force }),
  getContentUpdates: (instanceId: string) =>
    call<ContentUpdate[]>("get_content_updates", { instanceId }),
  applyContentUpdate: (instanceId: string, kind: string, fileName: string) =>
    call<string>("apply_content_update", { instanceId, kind, fileName }),
  planContentRemoval: (instanceId: string, kind: string, fileName: string) =>
    call<RemovalPlan>("plan_content_removal", { instanceId, kind, fileName }),
  getContentDependents: (instanceId: string, kind: string, fileName: string) =>
    call<string[]>("get_content_dependents", { instanceId, kind, fileName }),
  updateInstance: (
    instanceId: string,
    name: string,
    minMemoryMb: number | null,
    maxMemoryMb: number | null,
    javaPath: string | null,
    loader: string | null,
    loaderVersion: string | null,
    versionId: string,
  ) =>
    call<Instance>("update_instance", {
      instanceId,
      name,
      minMemoryMb,
      maxMemoryMb,
      javaPath,
      loader,
      loaderVersion,
      versionId,
    }),
  deleteInstance: (instanceId: string) =>
    call<void>("delete_instance", { instanceId }),
  listVersions: (includeSnapshots = false) =>
    call<VersionEntry[]>("list_versions", { includeSnapshots }),
  listInstalledVersions: () => call<string[]>("list_installed_versions"),
  getInstanceMedia: (instanceId: string) =>
    call<VersionMedia | null>("get_instance_media", { instanceId }),
  setInstanceBanner: (instanceId: string, sourcePath: string) =>
    call<VersionMedia>("set_instance_banner", { instanceId, sourcePath }),
  clearInstanceBanner: (instanceId: string) =>
    call<void>("clear_instance_banner", { instanceId }),
  setInstanceLogo: (instanceId: string, sourcePath: string) =>
    call<string>("set_instance_logo", { instanceId, sourcePath }),
  clearInstanceLogo: (instanceId: string) =>
    call<void>("clear_instance_logo", { instanceId }),
  backfillPackLogos: () => call<Instance[]>("backfill_pack_logos"),
  detectLaunchers: () => call<LauncherSource[]>("detect_launchers"),
  scanLauncher: (kind: string, root: string) =>
    call<MigrationScan>("scan_launcher", { kind, root }),
  migrateInstances: (kind: string, root: string, ids: string[]) =>
    call<MigrationOutcome>("migrate_instances", { kind, root, ids }),
  listTasks: () => call<Task[]>("list_tasks"),
  clearFinishedTasks: () => call<void>("clear_finished_tasks"),
  cancelTask: (taskId: string) => call<boolean>("cancel_task", { taskId }),
  recoverInterrupted: () => call<PendingOperation[]>("recover_interrupted"),
  installInstance: (instanceId: string) =>
    call<void>("install_instance", { instanceId }),
  getJavaStatus: (instanceId: string) =>
    call<JavaStatus>("get_java_status", { instanceId }),
  authBegin: () => call<DeviceCodeInfo>("auth_begin"),
  listAccounts: () => call<AccountView[]>("list_accounts"),
  setActiveAccount: (accountId: string) =>
    call<void>("set_active_account", { accountId }),
  removeAccount: (accountId: string) =>
    call<void>("remove_account", { accountId }),
  launchInstance: (instanceId: string) =>
    call<string>("launch_instance", { instanceId }),
  killInstance: (runningId: string) =>
    call<void>("kill_instance", { runningId }),
  listRunning: () => call<RunningInfo[]>("list_running"),
  getLogs: (runningId: string) => call<LogLine[]>("get_logs", { runningId }),
  closeRunning: (runningId: string) =>
    call<void>("close_running", { runningId }),
  getLogRecords: (limit?: number) =>
    call<LogRecord[]>("get_log_records", { limit: limit ?? null }),
  clearLogRecords: () => call<void>("clear_log_records"),
  getLogConfig: () => call<LogConfig>("get_log_config"),
  listInstanceLogs: (instanceId: string) =>
    call<InstanceLogFile[]>("list_instance_logs", { instanceId }),
  searchInstanceLog: (
    instanceId: string,
    name: string,
    query: string,
    minLevel: string | null = null,
  ) =>
    call<LogSearch>("search_instance_log", { instanceId, name, query, minLevel, limit: null }),
  deleteInstanceLog: (instanceId: string, name: string) =>
    call<void>("delete_instance_log", { instanceId, name }),
  setLogLevel: (level: string) => call<LogConfig>("set_log_level", { level }),
  checkForUpdates: () => call<UpdateInfo>("check_for_updates"),
  getAboutLinks: () => call<AboutLinks>("get_about_links"),
  getSystemStats: () => call<SystemStats>("get_system_stats"),
  getSystemUsage: () => call<SystemUsage>("get_system_usage"),
  previewLaunchArgs: (settings: LauncherSettings) =>
    call<LaunchPreview>("preview_launch_args", { settings }),
  getAppearance: () => call<Appearance>("get_appearance"),
  listSkins: () => call<SkinEntry[]>("list_skins"),
  addSkinFromFile: (path: string, name: string | null, variant: string) =>
    call<SkinEntry>("add_skin_from_file", { path, name, variant }),
  addSkinFromReference: (reference: string) =>
    call<SkinEntry>("add_skin_from_reference", { reference }),
  deleteSkin: (skinId: string) => call<void>("delete_skin", { skinId }),
  renameSkin: (skinId: string, name: string) =>
    call<SkinEntry>("rename_skin", { skinId, name }),
  getWornSkin: (uuid: string) => call<SkinEntry | null>("get_worn_skin", { uuid }),
  applySavedSkin: (skinId: string, variant: string | null = null) =>
    call<Appearance>("apply_saved_skin", { skinId, variant }),
  resetSkin: () => call<Appearance>("reset_skin"),
  setCape: (capeId: string | null) => call<Appearance>("set_cape", { capeId }),
};
