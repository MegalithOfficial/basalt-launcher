import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";

import { api } from "./lib/api";
import type { InstallState, Instance, LauncherSettings, View } from "./lib/types";

interface StagePayload {
  instance_id: string;
  stage: string;
}

interface ProgressPayload {
  instance_id: string;
  completed: number;
  total: number;
  downloaded_bytes: number;
  total_bytes: number;
  current: string;
}

const blankInstall: InstallState = {
  stage: "metadata",
  completed: 0,
  total: 0,
  downloadedBytes: 0,
  totalBytes: 0,
};

interface AppStore {
  view: View;
  ready: boolean;
  error: string | null;
  settings: LauncherSettings | null;
  instances: Instance[];
  installs: Record<string, InstallState>;
  installedIds: string[];

  setView: (view: View) => void;
  init: () => Promise<void>;
  refreshInstances: () => Promise<void>;
  createInstance: (name: string, versionId: string) => Promise<Instance>;
  deleteInstance: (id: string) => Promise<void>;
  installInstance: (id: string) => Promise<void>;
}

let listenersBound = false;

export const useStore = create<AppStore>((set) => ({
  view: "home",
  ready: false,
  error: null,
  settings: null,
  instances: [],
  installs: {},
  installedIds: [],

  setView: (view) => set({ view }),

  init: async () => {
    if (!listenersBound) {
      listenersBound = true;
      await listen<StagePayload>("install:stage", (e) => {
        const { instance_id, stage } = e.payload;
        set((s) => ({
          installs: {
            ...s.installs,
            [instance_id]: { ...(s.installs[instance_id] ?? blankInstall), stage },
          },
        }));
        if (stage === "done") {
          set((s) => ({
            installedIds: s.installedIds.includes(instance_id)
              ? s.installedIds
              : [...s.installedIds, instance_id],
          }));
          setTimeout(() => {
            set((s) => {
              const next = { ...s.installs };
              delete next[instance_id];
              return { installs: next };
            });
          }, 1400);
        }
      });
      await listen<ProgressPayload>("install:progress", (e) => {
        const p = e.payload;
        set((s) => ({
          installs: {
            ...s.installs,
            [p.instance_id]: {
              stage: s.installs[p.instance_id]?.stage ?? "downloading",
              completed: p.completed,
              total: p.total,
              downloadedBytes: p.downloaded_bytes,
              totalBytes: p.total_bytes,
            },
          },
        }));
      });
    }

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

  refreshInstances: async () => {
    set({ instances: await api.listInstances() });
  },

  createInstance: async (name, versionId) => {
    const instance = await api.createInstance(name, versionId);
    set((s) => ({ instances: [...s.instances, instance] }));
    return instance;
  },

  deleteInstance: async (id) => {
    await api.deleteInstance(id);
    set((s) => ({
      instances: s.instances.filter((i) => i.id !== id),
      installedIds: s.installedIds.filter((x) => x !== id),
    }));
  },

  installInstance: async (id) => {
    set((s) => ({ installs: { ...s.installs, [id]: { ...blankInstall } } }));
    try {
      await api.installInstance(id);
    } catch (e) {
      set((s) => {
        const next = { ...s.installs };
        delete next[id];
        return { installs: next, error: String(e) };
      });
    }
  },
}));
