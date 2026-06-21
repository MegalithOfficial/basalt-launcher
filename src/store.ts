import { create } from "zustand";

import { api } from "./lib/api";
import type { Instance, LauncherSettings, View } from "./lib/types";

interface AppStore {
  view: View;
  ready: boolean;
  error: string | null;
  settings: LauncherSettings | null;
  instances: Instance[];

  setView: (view: View) => void;
  init: () => Promise<void>;
}

export const useStore = create<AppStore>((set) => ({
  view: "home",
  ready: false,
  error: null,
  settings: null,
  instances: [],

  setView: (view) => set({ view }),

  init: async () => {
    try {
      const [settings, instances] = await Promise.all([
        api.getSettings(),
        api.listInstances(),
      ]);
      set({ settings, instances, ready: true, error: null });
    } catch (e) {
      set({ error: String(e), ready: true });
    }
  },
}));
