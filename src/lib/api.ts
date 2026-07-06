import { invoke } from "@tauri-apps/api/core";

import type {
  AccountView,
  DeviceCodeInfo,
  Instance,
  JavaStatus,
  LauncherSettings,
  LogLine,
  RunningInfo,
  VersionEntry,
  VersionMedia,
} from "./types";

export const api = {
  getSettings: () => invoke<LauncherSettings>("get_settings"),
  updateSettings: (settings: LauncherSettings) =>
    invoke<void>("update_settings", { settings }),
  listInstances: () => invoke<Instance[]>("list_instances"),
  createInstance: (name: string, versionId: string) =>
    invoke<Instance>("create_instance", { name, versionId }),
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
