import { invoke } from "@tauri-apps/api/core";

import type { Instance, JavaStatus, LauncherSettings, VersionEntry } from "./types";

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
  installInstance: (instanceId: string) =>
    invoke<void>("install_instance", { instanceId }),
  getJavaStatus: (instanceId: string) =>
    invoke<JavaStatus>("get_java_status", { instanceId }),
};
