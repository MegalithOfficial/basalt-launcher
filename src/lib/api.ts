import { invoke } from "@tauri-apps/api/core";

import { log } from "./log";
import type {
  AboutLinks,
  AccountView,
  AppUpdateStatus,
  BannerEntry,
  Appearance,
  AppInfo,
  Changelog,
  ContentItem,
  ContentUpdate,
  DeviceCodeInfo,
  Diagnosis,
  FilterTaxonomy,
  InstallPlan,
  InstalledFile,
  InstalledItem,
  Instance,
  InstanceGroup,
  InstanceOrganization,
  JavaInfo,
  JavaStatus,
  LauncherSource,
  NetworkProbe,
  LaunchPreview,
  MigrationOutcome,
  MigrationScan,
  LauncherSettings,
  InstanceLogFile,
  LogConfig,
  LogLine,
  LogSearch,
  LogRecord,
  ManualDownload,
  ManualDownloadSource,
  ModpackInstallPlan,
  ModpackUpgrade,
  ModpackUpgradePlan,
  PackExport,
  PackFormat,
  PackPreview,
  PlayStats,
  ProjectDetails,
  RemovalPlan,
  RepairReport,
  SnapshotSummary,
  Screenshot,
  WorldPacks,
  PathKind,
  StorageReport,
  ReclaimOutcome,
  Thumbnail,
  ProjectSummary,
  ProjectVersion,
  RunningInfo,
  SearchPage,
  SearchQuery,
  ConsoleLine,
  Server,
  ServerEntry,
  ServerFlavor,
  ServerSoftware,
  PlayerEntry,
  PlayerList,
  ServerFolder,
  ServerProperty,
  ServerRunningInfo,
  ServerText,
  TextProblem,
  PendingOperation,
  SkinEntry,
  SystemStats,
  SystemUsage,
  DataLocation,
  DataRoot,
  LocationCandidate,
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
  installJavaRuntime: (major: number, instanceId: string | null = null) =>
    call<JavaInfo>("install_java_runtime", { major, instanceId }),
  updateSettings: (settings: LauncherSettings) =>
    call<void>("update_settings", { settings }),
  listInstances: () => call<Instance[]>("list_instances"),
  getInstanceLaunchCommand: (instanceId: string) =>
    call<string>("get_instance_launch_command", { instanceId }),
  getPlayStats: (days: number | null, page: number | null = null) =>
    call<PlayStats>("get_play_stats", { days, page }),
  reconnectDiscord: () => call<void>("reconnect_discord"),
  getInstanceOrganization: () =>
    call<InstanceOrganization>("get_instance_organization"),
  createInstanceGroup: (name: string) =>
    call<InstanceGroup>("create_instance_group", { name }),
  renameInstanceGroup: (groupId: string, name: string) =>
    call<InstanceGroup>("rename_instance_group", { groupId, name }),
  deleteInstanceGroup: (groupId: string) =>
    call<void>("delete_instance_group", { groupId }),
  setInstanceNotes: (instanceId: string, notes: string) =>
    call<void>("set_instance_notes", { instanceId, notes }),
  setInstanceLaunchTools: (
    instanceId: string,
    wrapper: string,
    preLaunch: string,
    postExit: string,
  ) =>
    call<void>("set_instance_launch_tools", { instanceId, wrapper, preLaunch, postExit }),
  moveInstanceToGroup: (instanceId: string, groupId: string | null) =>
    call<void>("move_instance_to_group", { instanceId, groupId }),
  reorderInstanceGroups: (groupIds: string[]) =>
    call<void>("reorder_instance_groups", { groupIds }),
  reorderGroupInstances: (groupId: string | null, instanceIds: string[]) =>
    call<void>("reorder_group_instances", { groupId, instanceIds }),
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
  listInstanceDatapacks: (instanceId: string) =>
    call<WorldPacks[]>("list_instance_datapacks", { instanceId }),
  toggleDatapack: (instanceId: string, world: string, fileName: string) =>
    call<boolean>("toggle_datapack", { instanceId, world, fileName }),
  deleteDatapack: (instanceId: string, world: string, fileName: string) =>
    call<void>("delete_datapack", { instanceId, world, fileName }),
  addDatapacks: (instanceId: string, world: string, sources: string[]) =>
    call<number>("add_datapacks", { instanceId, world, sources }),
  installDatapack: (
    provider: string,
    projectId: string,
    instanceId: string,
    world: string,
    versionId: string | null = null,
  ) =>
    call<string[]>("install_datapack", {
      provider,
      projectId,
      instanceId,
      world,
      versionId,
    }),
  checkDatapackUpdates: (instanceId: string) =>
    call<number>("check_datapack_updates", { instanceId }),
  applyDatapackUpdate: (instanceId: string, world: string, fileName: string) =>
    call<string[]>("apply_datapack_update", { instanceId, world, fileName }),
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
  planModpackInstall: (
    provider: string,
    projectId: string,
    versionId: string,
    manualDownloads: ManualDownloadSource[] = [],
  ) =>
    call<ModpackInstallPlan>("plan_modpack_install", {
      provider,
      projectId,
      versionId,
      manualDownloads,
    }),
  findCurseforgeDownload: (download: ManualDownload, startedAtMs: number) =>
    call<string | null>("find_curseforge_download", { download, startedAtMs }),
  installModpack: (
    provider: string,
    projectId: string,
    versionId: string,
    manualDownloads: ManualDownloadSource[] = [],
  ) =>
    call<Instance>("install_modpack", {
      provider,
      projectId,
      versionId,
      manualDownloads,
    }),
  checkModpackUpgrade: (instanceId: string) =>
    call<ModpackUpgrade | null>("check_modpack_upgrade", { instanceId }),
  planModpackUpgrade: (
    instanceId: string,
    targetVersionId: string,
    manualDownloads: ManualDownloadSource[] = [],
  ) =>
    call<ModpackUpgradePlan>("plan_modpack_upgrade", {
      instanceId,
      targetVersionId,
      manualDownloads,
    }),
  upgradeModpack: (
    instanceId: string,
    targetVersionId: string,
    manualDownloads: ManualDownloadSource[] = [],
    snapshotFirst = true,
  ) =>
    call<Instance>("upgrade_modpack", {
      instanceId,
      targetVersionId,
      manualDownloads,
      snapshotFirst,
    }),
  linkModpack: (
    instanceId: string,
    provider: string,
    projectId: string,
    versionId: string,
  ) => call<Instance>("link_modpack", { instanceId, provider, projectId, versionId }),
  unlinkModpack: (instanceId: string) =>
    call<Instance>("unlink_modpack", { instanceId }),
  checkContentUpdates: (instanceId: string, force = false) =>
    call<ContentUpdate[]>("check_content_updates", { instanceId, force }),
  getContentUpdates: (instanceId: string) =>
    call<ContentUpdate[]>("get_content_updates", { instanceId }),
  planContentUpdate: (instanceId: string, kind: string, fileName: string) =>
    call<ManualDownload | null>("plan_content_update", { instanceId, kind, fileName }),
  applyContentUpdate: (
    instanceId: string,
    kind: string,
    fileName: string,
    manualDownloads: ManualDownloadSource[] = [],
  ) =>
    call<string>("apply_content_update", { instanceId, kind, fileName, manualDownloads }),
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
    jvmArgs: string | null = null,
    jvmArgsMode: string | null = null,
    envVars: string | null = null,
    envVarsMode: string | null = null,
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
      jvmArgs,
      jvmArgsMode,
      envVars,
      envVarsMode,
    }),
  deleteInstance: (instanceId: string) =>
    call<void>("delete_instance", { instanceId }),
  repairInstance: (instanceId: string) =>
    call<RepairReport>("repair_instance", { instanceId }),
  duplicateInstance: (instanceId: string) =>
    call<Instance>("duplicate_instance", { instanceId }),
  instanceSnapshotUsage: (instanceId: string) =>
    call<number>("instance_snapshot_usage", { instanceId }),
  listInstanceSnapshots: (instanceId: string) =>
    call<SnapshotSummary[]>("list_instance_snapshots", { instanceId }),
  createInstanceSnapshot: (
    instanceId: string,
    name: string | null = null,
    excluded: string[] = [],
  ) => call<SnapshotSummary>("create_instance_snapshot", { instanceId, name, excluded }),
  renameInstanceSnapshot: (instanceId: string, snapshotId: string, name: string) =>
    call<SnapshotSummary>("rename_instance_snapshot", { instanceId, snapshotId, name }),
  deleteInstanceSnapshot: (instanceId: string, snapshotId: string) =>
    call<void>("delete_instance_snapshot", { instanceId, snapshotId }),
  restoreInstanceSnapshot: (instanceId: string, snapshotId: string) =>
    call<SnapshotSummary>("restore_instance_snapshot", { instanceId, snapshotId }),
  listVersions: (includeSnapshots = false) =>
    call<VersionEntry[]>("list_versions", { includeSnapshots }),
  listInstalledVersions: () => call<string[]>("list_installed_versions"),
  getInstanceMedia: (instanceId: string) =>
    call<VersionMedia | null>("get_instance_media", { instanceId }),
  setInstanceBanner: (instanceId: string, sourcePath: string) =>
    call<VersionMedia>("set_instance_banner", { instanceId, sourcePath }),
  clearInstanceBanner: (instanceId: string) =>
    call<void>("clear_instance_banner", { instanceId }),
  listBannerLibrary: () => call<BannerEntry[]>("list_banner_library"),
  addBannerToLibrary: (sourcePath: string) =>
    call<BannerEntry>("add_banner_to_library", { sourcePath }),
  deleteBanner: (bannerId: string) => call<void>("delete_banner", { bannerId }),
  applyBanner: (instanceId: string, bannerId: string) =>
    call<VersionMedia>("apply_banner", { instanceId, bannerId }),
  applyLogo: (instanceId: string, bannerId: string) =>
    call<string>("apply_logo", { instanceId, bannerId }),
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
  inspectPackFile: (path: string) => call<PackPreview>("inspect_pack_file", { path }),
  inspectPackwizUrl: (url: string) => call<PackPreview>("inspect_packwiz_url", { url }),
  importPackFile: (path: string, name: string | null) =>
    call<Instance>("import_pack_file", { path, name }),
  importPackwizUrl: (url: string, name: string | null) =>
    call<Instance>("import_packwiz_url", { url, name }),
  exportInstancePack: (instanceId: string, format: PackFormat, path: string) =>
    call<PackExport>("export_instance_pack", { instanceId, format, path }),
  packExportName: (name: string, format: PackFormat) =>
    call<string>("pack_export_name", { name, format }),
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
    crash: boolean,
    query: string,
    minLevel: string | null = null,
  ) =>
    call<LogSearch>("search_instance_log", {
      instanceId,
      name,
      crash,
      query,
      minLevel,
      limit: 1500,
    }),
  diagnoseInstance: (instanceId: string, name: string | null = null, crash = false) =>
    call<Diagnosis[]>("diagnose_instance", { instanceId, name, crash }),
  redactInstanceLog: (instanceId: string, name: string, crash: boolean) =>
    call<string>("redact_instance_log", { instanceId, name, crash }),
  redactText: (text: string) => call<string>("redact_text", { text }),
  shareLog: (text: string) => call<string>("share_log", { text }),
  deleteInstanceLog: (instanceId: string, name: string, crash: boolean) =>
    call<void>("delete_instance_log", { instanceId, name, crash }),
  listScreenshots: (instanceId: string) =>
    call<Screenshot[]>("list_screenshots", { instanceId }),
  deleteScreenshots: (instanceId: string, names: string[]) =>
    call<number>("delete_screenshots", { instanceId, names }),
  copyScreenshot: (instanceId: string, name: string) =>
    call<void>("copy_screenshot", { instanceId, name }),
  ensureThumbnails: (instanceId: string, names: string[]) =>
    call<Thumbnail[]>("ensure_thumbnails", { instanceId, names }),
  setLogLevel: (level: string) => call<LogConfig>("set_log_level", { level }),
  resetLauncher: (deep: boolean) => call<void>("reset_launcher", { deep }),
  testNetwork: (url?: string) => call<NetworkProbe>("test_network", { url: url ?? null }),
  checkForUpdates: () => call<UpdateInfo>("check_for_updates"),
  getAppUpdateStatus: () => call<AppUpdateStatus>("get_app_update_status"),
  dismissAppUpdate: (version: string) =>
    call<AppUpdateStatus>("dismiss_app_update", { version }),
  downloadAppUpdate: () => call<AppUpdateStatus>("download_app_update"),
  installAppUpdate: () => call<void>("install_app_update"),
  getAboutLinks: () => call<AboutLinks>("get_about_links"),
  scanStorage: (force = false) => call<StorageReport>("scan_storage", { force }),
  reclaimStorage: (targets: string[]) =>
    call<ReclaimOutcome>("reclaim_storage", { targets }),
  inspectPaths: (paths: string[]) => call<PathKind[]>("inspect_paths", { paths }),
  listServers: () => call<Server[]>("list_servers"),
  listServerSoftware: () => call<ServerSoftware[]>("list_server_software"),
  listServerContent: (serverId: string, reconcile?: boolean) =>
    call<ContentItem[]>("list_server_content", { serverId, reconcile }),
  planServerContentRemoval: (serverId: string, fileName: string) =>
    call<RemovalPlan>("plan_server_content_removal", { serverId, fileName }),
  toggleServerContent: (serverId: string, fileName: string) =>
    call<boolean>("toggle_server_content", { serverId, fileName }),
  deleteServerContent: (serverId: string, fileName: string) =>
    call<void>("delete_server_content", { serverId, fileName }),
  addServerContent: (serverId: string, sources: string[]) =>
    call<number>("add_server_content", { serverId, sources }),
  listServerPlayers: (serverId: string, list: PlayerList) =>
    call<PlayerEntry[]>("list_server_players", { serverId, list }),
  addServerPlayer: (serverId: string, list: PlayerList, name: string, reason: string | null) =>
    call<void>("add_server_player", { serverId, list, name, reason }),
  removeServerPlayer: (serverId: string, list: PlayerList, name: string) =>
    call<void>("remove_server_player", { serverId, list, name }),
  setServerWhitelist: (serverId: string, enabled: boolean) =>
    call<void>("set_server_whitelist", { serverId, enabled }),
  checkServerContentUpdates: (serverId: string, force?: boolean) =>
    call<ContentUpdate[]>("check_server_content_updates", { serverId, force }),
  planServerContentInstall: (
    serverId: string,
    provider: string,
    projectId: string,
    versionId: string | null,
  ) =>
    call<InstallPlan>("plan_server_content_install", {
      serverId,
      provider,
      projectId,
      versionId,
    }),
  installServerContent: (
    serverId: string,
    provider: string,
    projectId: string,
    versionId: string | null,
    withDependencies: boolean,
  ) =>
    call<InstalledItem[]>("install_server_content", {
      serverId,
      provider,
      projectId,
      versionId,
      withDependencies,
    }),
  listServerFlavorVersions: (flavor: ServerFlavor, versionId: string) =>
    call<string[]>("list_server_flavor_versions", { flavor, versionId }),
  createServer: (
    name: string,
    flavor: ServerFlavor,
    versionId: string,
    flavorVersion: string | null,
    acceptEula: boolean,
  ) => call<Server>("create_server", { name, flavor, versionId, flavorVersion, acceptEula }),
  inspectServerFolder: (path: string) => call<ServerFolder>("inspect_server_folder", { path }),
  importServer: (
    path: string,
    name: string,
    flavor: ServerFlavor,
    versionId: string,
    flavorVersion: string | null,
    acceptEula: boolean,
  ) => call<Server>("import_server", { path, name, flavor, versionId, flavorVersion, acceptEula }),
  installServer: (serverId: string) => call<Server>("install_server", { serverId }),
  updateServerSettings: (
    serverId: string,
    name: string,
    versionId: string,
    flavorVersion: string | null,
    minMemoryMb: number | null,
    maxMemoryMb: number | null,
    javaPath: string | null,
    jvmArgs: string | null,
    jvmArgsMode: string | null,
    stopTimeoutSecs: number | null,
    notes: string | null,
  ) =>
    call<Server>("update_server_settings", {
      serverId,
      name,
      versionId,
      flavorVersion,
      minMemoryMb,
      maxMemoryMb,
      javaPath,
      jvmArgs,
      jvmArgsMode,
      stopTimeoutSecs,
      notes,
    }),
  getServerLaunchCommand: (serverId: string) =>
    call<string>("get_server_launch_command", { serverId }),
  acceptServerEula: (serverId: string) => call<Server>("accept_server_eula", { serverId }),
  deleteServer: (serverId: string, deleteFiles: boolean) =>
    call<void>("delete_server", { serverId, deleteFiles }),
  startServer: (serverId: string) => call<ServerRunningInfo>("start_server", { serverId }),
  stopServer: (serverId: string) => call<void>("stop_server", { serverId }),
  restartServer: (serverId: string) => call<ServerRunningInfo>("restart_server", { serverId }),
  forceStopServer: (serverId: string) => call<void>("force_stop_server", { serverId }),
  getServerDiskUsage: (serverId: string) => call<number>("get_server_disk_usage", { serverId }),
  sendServerCommand: (serverId: string, line: string) =>
    call<void>("send_server_command", { serverId, line }),
  getServerConsole: (serverId: string) => call<ConsoleLine[]>("get_server_console", { serverId }),
  listRunningServers: () => call<ServerRunningInfo[]>("list_running_servers"),
  getServerProperties: (serverId: string) =>
    call<ServerProperty[]>("get_server_properties", { serverId }),
  setServerProperties: (serverId: string, changes: ServerProperty[], removed: string[]) =>
    call<ServerProperty[]>("set_server_properties", { serverId, changes, removed }),
  listServerFiles: (serverId: string, path: string) =>
    call<ServerEntry[]>("list_server_files", { serverId, path }),
  readServerFile: (serverId: string, path: string) =>
    call<ServerText>("read_server_file", { serverId, path }),
  writeServerFile: (serverId: string, path: string, text: string) =>
    call<TextProblem | null>("write_server_file", { serverId, path, text }),
  checkServerFile: (path: string, text: string) =>
    call<TextProblem | null>("check_server_file", { path, text }),
  createServerFolder: (serverId: string, path: string, name: string) =>
    call<string>("create_server_folder", { serverId, path, name }),
  renameServerEntry: (serverId: string, path: string, name: string) =>
    call<string>("rename_server_entry", { serverId, path, name }),
  deleteServerEntry: (serverId: string, path: string) =>
    call<void>("delete_server_entry", { serverId, path }),
  uploadServerFiles: (serverId: string, path: string, sources: string[]) =>
    call<number>("upload_server_files", { serverId, path, sources }),
  openFolder: (path: string) => call<void>("open_folder", { path }),
  openFile: (path: string) => call<void>("open_file", { path }),
  getLanAddress: () => call<string | null>("get_lan_address"),
  getSystemStats: () => call<SystemStats>("get_system_stats"),
  getSystemUsage: () => call<SystemUsage>("get_system_usage"),
  getDataLocations: () => call<DataLocation[]>("get_data_locations"),
  inspectDataLocation: (slot: DataRoot, path: string) =>
    call<LocationCandidate>("inspect_data_location", { slot, path }),
  setDataLocation: (slot: DataRoot, path: string | null, moveExisting: boolean) =>
    call<void>("set_data_location", { slot, path, moveExisting }),
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
