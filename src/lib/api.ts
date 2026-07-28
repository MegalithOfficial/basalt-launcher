import { invoke } from "@tauri-apps/api/core";

import type {
  AccountView,
  AppInfo,
  Changelog,
  ContentItem,
  ContentUpdate,
  DeviceCodeInfo,
  FilterTaxonomy,
  InstallPlan,
  InstalledFile,
  Instance,
  JavaInfo,
  JavaStatus,
  LauncherSettings,
  LogLine,
  ProjectDetails,
  ProjectSummary,
  ProjectVersion,
  RunningInfo,
  SearchPage,
  SearchQuery,
  VersionEntry,
  VersionMedia,
} from "./types";

export const api = {
  getSettings: () => invoke<LauncherSettings>("get_settings"),
  getAppInfo: () => invoke<AppInfo>("get_app_info"),
  listJavas: () => invoke<JavaInfo[]>("list_javas"),
  updateSettings: (settings: LauncherSettings) =>
    invoke<void>("update_settings", { settings }),
  listInstances: () => invoke<Instance[]>("list_instances"),
  createInstance: (
    name: string,
    versionId: string,
    loader: string | null = null,
    loaderVersion: string | null = null,
  ) =>
    invoke<Instance>("create_instance", { name, versionId, loader, loaderVersion }),
  listLoaderVersions: (loader: string, gameVersion: string) =>
    invoke<string[]>("list_loader_versions", { loader, gameVersion }),
  listInstanceContent: (instanceId: string, kind: string, reconcile = false) =>
    invoke<ContentItem[]>("list_instance_content", { instanceId, kind, reconcile }),
  toggleInstanceContent: (instanceId: string, kind: string, fileName: string) =>
    invoke<boolean>("toggle_instance_content", { instanceId, kind, fileName }),
  deleteInstanceContent: (instanceId: string, kind: string, fileName: string) =>
    invoke<void>("delete_instance_content", { instanceId, kind, fileName }),
  addInstanceContent: (instanceId: string, kind: string, sources: string[]) =>
    invoke<number>("add_instance_content", { instanceId, kind, sources }),
  searchContent: (provider: string, kind: string, query: SearchQuery) =>
    invoke<SearchPage>("search_content", { provider, kind, query }),
  getFilterTaxonomy: (provider: string, kind: string, includeSnapshots = false) =>
    invoke<FilterTaxonomy>("get_filter_taxonomy", { provider, kind, includeSnapshots }),
  getVersionChangelog: (provider: string, projectId: string, versionId: string) =>
    invoke<Changelog>("get_version_changelog", { provider, projectId, versionId }),
  resolveProjects: (provider: string, ids: string[]) =>
    invoke<ProjectSummary[]>("resolve_projects", { provider, ids }),
  getInstalledProjectFile: (instanceId: string, kind: string, projectId: string) =>
    invoke<InstalledFile | null>("get_installed_project_file", {
      instanceId,
      kind,
      projectId,
    }),
  getProjectDetails: (provider: string, projectId: string) =>
    invoke<ProjectDetails>("get_project_details", { provider, projectId }),
  listProjectVersions: (
    provider: string,
    projectId: string,
    kind: string,
    gameVersion: string,
    loader: string | null,
  ) =>
    invoke<ProjectVersion[]>("list_project_versions", {
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
    invoke<InstallPlan>("plan_content_install", {
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
    invoke<string[]>("install_content", {
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
    invoke<Instance>("install_modpack", { provider, projectId, versionId }),
  checkContentUpdates: (instanceId: string, force = false) =>
    invoke<ContentUpdate[]>("check_content_updates", { instanceId, force }),
  getContentUpdates: (instanceId: string) =>
    invoke<ContentUpdate[]>("get_content_updates", { instanceId }),
  applyContentUpdate: (instanceId: string, kind: string, fileName: string) =>
    invoke<string>("apply_content_update", { instanceId, kind, fileName }),
  getContentDependents: (instanceId: string, kind: string, fileName: string) =>
    invoke<string[]>("get_content_dependents", { instanceId, kind, fileName }),
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
    invoke<Instance>("update_instance", {
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
    invoke<void>("delete_instance", { instanceId }),
  listVersions: (includeSnapshots = false) =>
    invoke<VersionEntry[]>("list_versions", { includeSnapshots }),
  listInstalledVersions: () => invoke<string[]>("list_installed_versions"),
  getInstanceMedia: (instanceId: string) =>
    invoke<VersionMedia | null>("get_instance_media", { instanceId }),
  setInstanceBanner: (instanceId: string, sourcePath: string) =>
    invoke<VersionMedia>("set_instance_banner", { instanceId, sourcePath }),
  clearInstanceBanner: (instanceId: string) =>
    invoke<void>("clear_instance_banner", { instanceId }),
  setInstanceLogo: (instanceId: string, sourcePath: string) =>
    invoke<string>("set_instance_logo", { instanceId, sourcePath }),
  clearInstanceLogo: (instanceId: string) =>
    invoke<void>("clear_instance_logo", { instanceId }),
  backfillPackLogos: () => invoke<Instance[]>("backfill_pack_logos"),
  installInstance: (instanceId: string) =>
    invoke<void>("install_instance", { instanceId }),
  getJavaStatus: (instanceId: string) =>
    invoke<JavaStatus>("get_java_status", { instanceId }),
  authBegin: () => invoke<DeviceCodeInfo>("auth_begin"),
  listAccounts: () => invoke<AccountView[]>("list_accounts"),
  setActiveAccount: (accountId: string) =>
    invoke<void>("set_active_account", { accountId }),
  removeAccount: (accountId: string) =>
    invoke<void>("remove_account", { accountId }),
  launchInstance: (instanceId: string) =>
    invoke<string>("launch_instance", { instanceId }),
  killInstance: (runningId: string) =>
    invoke<void>("kill_instance", { runningId }),
  listRunning: () => invoke<RunningInfo[]>("list_running"),
  getLogs: (runningId: string) => invoke<LogLine[]>("get_logs", { runningId }),
  closeRunning: (runningId: string) =>
    invoke<void>("close_running", { runningId }),
};
