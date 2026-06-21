import { invoke } from "@tauri-apps/api/core";

import type { Instance, LauncherSettings, VersionEntry } from "./types";

export const api = {
  getSettings: () => invoke<LauncherSettings>("get_settings"),
  updateSettings: (settings: LauncherSettings) =>
    invoke<void>("update_settings", { settings }),
  listInstances: () => invoke<Instance[]>("list_instances"),
  listVersions: (includeSnapshots = false) =>
    invoke<VersionEntry[]>("list_versions", { includeSnapshots }),
};
