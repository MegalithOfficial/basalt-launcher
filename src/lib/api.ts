import { invoke } from "@tauri-apps/api/core";

import type {
  AccountView,
  Changelog,
  ContentItem,
  DeviceCodeInfo,
  Instance,
  JavaStatus,
  LauncherSettings,
  LogLine,
  ProjectDetails,
  ProjectVersion,
  RunningInfo,
  SearchResult,
  VersionEntry,
  VersionMedia,
} from "./types";

export const api = {
  getSettings: () => invoke<LauncherSettings>("get_settings"),
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
  listInstanceContent: (instanceId: string, kind: string) =>
    invoke<ContentItem[]>("list_instance_content", { instanceId, kind }),
  toggleInstanceContent: (instanceId: string, kind: string, fileName: string) =>
    invoke<boolean>("toggle_instance_content", { instanceId, kind, fileName }),
  deleteInstanceContent: (instanceId: string, kind: string, fileName: string) =>
    invoke<void>("delete_instance_content", { instanceId, kind, fileName }),
  addInstanceContent: (instanceId: string, kind: string, sources: string[]) =>
    invoke<number>("add_instance_content", { instanceId, kind, sources }),
  searchContent: (
    provider: string,
    kind: string,
    query: string,
    gameVersion: string,
    loader: string | null,
  ) =>
    invoke<SearchResult[]>("search_content", { provider, kind, query, gameVersion, loader }),
  getVersionChangelog: (provider: string, projectId: string, versionId: string) =>
    invoke<Changelog>("get_version_changelog", { provider, projectId, versionId }),
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
  installContent: (
    provider: string,
    projectId: string,
    instanceId: string,
    kind: string,
    gameVersion: string,
    loader: string | null,
    versionId: string | null = null,
    title: string | null = null,
    iconUrl: string | null = null,
  ) =>
    invoke<string>("install_content", {
      provider,
      projectId,
      instanceId,
      kind,
      gameVersion,
      loader,
      versionId,
      title,
      iconUrl,
    }),
  updateInstance: (
    instanceId: string,
    name: string,
    minMemoryMb: number | null,
    maxMemoryMb: number | null,
    javaPath: string | null,
    loader: string | null,
    loaderVersion: string | null,
  ) =>
    invoke<Instance>("update_instance", {
      instanceId,
      name,
      minMemoryMb,
      maxMemoryMb,
      javaPath,
      loader,
      loaderVersion,
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
