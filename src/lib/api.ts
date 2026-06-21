import { invoke } from "@tauri-apps/api/core";

import type { Instance, LauncherSettings } from "./types";

/** Thin typed wrappers around the Rust command surface. */
export const api = {
  getSettings: () => invoke<LauncherSettings>("get_settings"),
  updateSettings: (settings: LauncherSettings) =>
    invoke<void>("update_settings", { settings }),
  listInstances: () => invoke<Instance[]>("list_instances"),
};
