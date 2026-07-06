import { listen } from "@tauri-apps/api/event";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { create } from "zustand";

import { api } from "./lib/api";
import { isInstanceInstalled } from "./lib/loader";
import type {
  AccountView,
  ContentKind,
  InstallState,
  Instance,
  LauncherSettings,
  LogLine,
  RunningInfo,
  SearchProvider,
  VersionMedia,
  View,
} from "./lib/types";

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

interface AuthPayload {
  status: "success" | "error";
  account?: AccountView;
  message?: string;
}

interface LogPayload {
  running_id: string;
  stream: string;
  line: string;
}

export interface AuthFlow {
  status: "idle" | "starting" | "pending" | "error";
  userCode?: string;
  verificationUri?: string;
  message?: string;
}

interface AppStore {
  view: View;
  ready: boolean;
  error: string | null;
  settings: LauncherSettings | null;
  instances: Instance[];
  installs: Record<string, InstallState>;
  installedIds: string[];
  accounts: AccountView[];
  auth: AuthFlow;
  running: Record<string, RunningInfo>;
  logs: Record<string, LogLine[]>;
  activeRunningId: string | null;
  media: Record<string, VersionMedia | null>;
  detailInstanceId: string | null;
  viewStack: View[];
  searchKind: ContentKind | null;
  projectRef: { provider: SearchProvider; id: string } | null;
  contentSources: Record<string, Record<string, { file_name: string; version_id: string | null }>>;
  installingContent: string[];
  selectedInstanceId: string | null;

  setView: (view: View) => void;
  init: () => Promise<void>;
  refreshInstances: () => Promise<void>;
  createInstance: (
    name: string,
    versionId: string,
    loader?: string | null,
    loaderVersion?: string | null,
  ) => Promise<Instance>;
  updateInstance: (
    id: string,
    name: string,
    minMemoryMb: number | null,
    maxMemoryMb: number | null,
    javaPath: string | null,
    loader: string | null,
    loaderVersion: string | null,
    versionId: string,
  ) => Promise<void>;
  deleteInstance: (id: string) => Promise<void>;
  installInstance: (id: string) => Promise<void>;
  refreshAccounts: () => Promise<void>;
  addAccount: () => Promise<void>;
  setActiveAccount: (id: string) => Promise<void>;
  removeAccount: (id: string) => Promise<void>;
  resetAuth: () => void;
  launchInstance: (id: string) => Promise<void>;
  killInstance: (runningId: string) => Promise<void>;
  closeRunning: (runningId: string) => Promise<void>;
  openConsole: (runningId: string) => void;
  loadMedia: (instanceId: string) => Promise<void>;
  selectInstance: (id: string) => void;
  openInstance: (id: string) => void;
  openSearch: (kind: ContentKind) => void;
  openProject: (provider: SearchProvider, id: string, kind?: ContentKind) => void;
  goBack: () => void;
  refreshContentSources: (instanceId: string, kind: string) => Promise<void>;
  installContent: (params: {
    provider: SearchProvider;
    projectId: string;
    instanceId: string;
    kind: string;
    gameVersion: string;
    loader: string | null;
    versionId?: string | null;
    title?: string | null;
    iconUrl?: string | null;
    withDependencies?: boolean;
  }) => Promise<string[]>;
  pickBanner: (instanceId: string) => Promise<void>;
  clearBanner: (instanceId: string) => Promise<void>;
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
  accounts: [],
  auth: { status: "idle" },
  running: {},
  logs: {},
  activeRunningId: null,
  media: {},
  selectedInstanceId: null,
  detailInstanceId: null,
  viewStack: [],
  searchKind: null,
  projectRef: null,
  contentSources: {},
  installingContent: [],

  setView: (view) =>
    set((s) => ({
      view,
      viewStack: s.view !== view ? [...s.viewStack.slice(-19), s.view] : s.viewStack,
    })),

  goBack: () =>
    set((s) => ({
      view: s.viewStack[s.viewStack.length - 1] ?? "home",
      viewStack: s.viewStack.slice(0, -1),
    })),

  openSearch: (kind) =>
    set((s) => ({
      searchKind: kind,
      view: "search",
      viewStack: s.view !== "search" ? [...s.viewStack.slice(-19), s.view] : s.viewStack,
    })),

  refreshContentSources: async (instanceId, kind) => {
    try {
      const entries = await api.listContentSources(instanceId, kind);
      const map: Record<string, { file_name: string; version_id: string | null }> = {};
      entries.forEach((e) => {
        map[e.project_id] = { file_name: e.file_name, version_id: e.version_id };
      });
      set((s) => ({
        contentSources: { ...s.contentSources, [`${instanceId}:${kind}`]: map },
      }));
    } catch {
      return;
    }
  },

  installContent: async (params) => {
    const key = `${params.instanceId}:${params.kind}:${params.projectId}`;
    set((s) => ({ installingContent: [...s.installingContent, key] }));
    try {
      const files = await api.installContent(
        params.provider,
        params.projectId,
        params.instanceId,
        params.kind,
        params.gameVersion,
        params.loader,
        params.versionId ?? null,
        params.title ?? null,
        params.iconUrl ?? null,
        params.withDependencies ?? true,
      );
      await useStore.getState().refreshContentSources(params.instanceId, params.kind);
      return files;
    } finally {
      set((s) => ({
        installingContent: s.installingContent.filter((k) => k !== key),
      }));
    }
  },

  openProject: (provider, id, kind) =>
    set((s) => ({
      projectRef: { provider, id },
      searchKind: kind ?? s.searchKind,
      view: "project",
      viewStack: s.view !== "project" ? [...s.viewStack.slice(-19), s.view] : s.viewStack,
    })),

  init: async () => {
    if (!listenersBound) {
      listenersBound = true;
      await listen<AuthPayload>("auth:state", (e) => {
        const p = e.payload;
        if (p.status === "success") {
          set({ auth: { status: "idle" } });
          void useStore.getState().refreshAccounts();
        } else {
          set({ auth: { status: "error", message: p.message } });
        }
      });
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
      await listen<LogPayload>("process:log", (e) => {
        const p = e.payload;
        set((s) => {
          const prev = s.logs[p.running_id] ?? [];
          const next = [...prev, { stream: p.stream, line: p.line }];
          if (next.length > 6000) next.splice(0, next.length - 6000);
          return { logs: { ...s.logs, [p.running_id]: next } };
        });
      });
      await listen<RunningInfo>("process:state", (e) => {
        const info = e.payload;
        set((s) => ({ running: { ...s.running, [info.running_id]: info } }));
        if (info.state !== "running") {
          void useStore.getState().refreshInstances();
        }
      });
    }

    try {
      const [settings, instances, accounts, installedVersions] = await Promise.all([
        api.getSettings(),
        api.listInstances(),
        api.listAccounts(),
        api.listInstalledVersions(),
      ]);
      const installedIds = instances
        .filter((i) => isInstanceInstalled(i, installedVersions))
        .map((i) => i.id);
      set((s) => ({
        settings,
        instances,
        accounts,
        installedIds,
        ready: true,
        error: null,
        selectedInstanceId: s.selectedInstanceId ?? instances[0]?.id ?? null,
      }));
    } catch (e) {
      set({ error: String(e), ready: true });
    }
  },

  refreshAccounts: async () => {
    set({ accounts: await api.listAccounts() });
  },

  addAccount: async () => {
    set({ auth: { status: "starting" } });
    try {
      const info = await api.authBegin();
      set({
        auth: {
          status: "pending",
          userCode: info.user_code,
          verificationUri: info.verification_uri,
          message: info.message,
        },
      });
    } catch (e) {
      set({ auth: { status: "error", message: String(e) } });
    }
  },

  setActiveAccount: async (id) => {
    await api.setActiveAccount(id);
    await useStore.getState().refreshAccounts();
  },

  removeAccount: async (id) => {
    await api.removeAccount(id);
    await useStore.getState().refreshAccounts();
  },

  resetAuth: () => set({ auth: { status: "idle" } }),

  launchInstance: async (id) => {
    const runningId = await api.launchInstance(id);
    set((s) => ({
      activeRunningId: runningId,
      view: "console",
      viewStack: s.view !== "console" ? [...s.viewStack.slice(-19), s.view] : s.viewStack,
    }));
    const backfill = await api.getLogs(runningId);
    set((s) => {
      const streamed = s.logs[runningId] ?? [];
      const merged = backfill.length >= streamed.length ? backfill : streamed;
      return { logs: { ...s.logs, [runningId]: merged } };
    });
  },

  killInstance: async (runningId) => {
    await api.killInstance(runningId);
  },

  closeRunning: async (runningId) => {
    await api.closeRunning(runningId);
    set((s) => {
      const running = { ...s.running };
      const logs = { ...s.logs };
      delete running[runningId];
      delete logs[runningId];
      return {
        running,
        logs,
        activeRunningId: s.activeRunningId === runningId ? null : s.activeRunningId,
        view: s.activeRunningId === runningId ? "home" : s.view,
      };
    });
  },

  openConsole: (runningId) =>
    set((s) => ({
      activeRunningId: runningId,
      view: "console",
      viewStack: s.view !== "console" ? [...s.viewStack.slice(-19), s.view] : s.viewStack,
    })),

  selectInstance: (id) => set({ selectedInstanceId: id }),

  openInstance: (id) =>
    set((s) => ({
      detailInstanceId: id,
      view: "instance",
      viewStack: s.view !== "instance" ? [...s.viewStack.slice(-19), s.view] : s.viewStack,
    })),

  loadMedia: async (instanceId) => {
    if (instanceId in useStore.getState().media) return;
    set((s) => ({ media: { ...s.media, [instanceId]: null } }));
    try {
      const media = await api.getInstanceMedia(instanceId);
      set((s) => ({ media: { ...s.media, [instanceId]: media } }));
    } catch {
      set((s) => ({ media: { ...s.media, [instanceId]: null } }));
    }
  },

  pickBanner: async (instanceId) => {
    const file = await openFileDialog({
      multiple: false,
      directory: false,
      filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "webp", "gif"] }],
    });
    if (typeof file !== "string") return;
    const media = await api.setInstanceBanner(instanceId, file);
    set((s) => ({ media: { ...s.media, [instanceId]: media } }));
  },

  clearBanner: async (instanceId) => {
    await api.clearInstanceBanner(instanceId);
    const media = await api.getInstanceMedia(instanceId).catch(() => null);
    set((s) => ({ media: { ...s.media, [instanceId]: media } }));
  },

  refreshInstances: async () => {
    set({ instances: await api.listInstances() });
  },

  createInstance: async (name, versionId, loader, loaderVersion) => {
    const instance = await api.createInstance(name, versionId, loader, loaderVersion);
    const installedVersions = await api.listInstalledVersions();
    set((s) => ({
      instances: [...s.instances, instance],
      selectedInstanceId: instance.id,
      installedIds: isInstanceInstalled(instance, installedVersions)
        ? [...s.installedIds, instance.id]
        : s.installedIds,
    }));
    return instance;
  },

  updateInstance: async (
    id,
    name,
    minMemoryMb,
    maxMemoryMb,
    javaPath,
    loader,
    loaderVersion,
    versionId,
  ) => {
    const updated = await api.updateInstance(
      id,
      name,
      minMemoryMb,
      maxMemoryMb,
      javaPath,
      loader,
      loaderVersion,
      versionId,
    );
    const installedVersions = await api.listInstalledVersions();
    set((s) => {
      const instances = s.instances.map((i) => (i.id === id ? updated : i));
      const media = { ...s.media };
      delete media[id];
      return {
        instances,
        media,
        installedIds: instances
          .filter((i) => isInstanceInstalled(i, installedVersions))
          .map((i) => i.id),
      };
    });
    void useStore.getState().loadMedia(id);
  },

  deleteInstance: async (id) => {
    await api.deleteInstance(id);
    set((s) => {
      const instances = s.instances.filter((i) => i.id !== id);
      return {
        instances,
        installedIds: s.installedIds.filter((x) => x !== id),
        selectedInstanceId:
          s.selectedInstanceId === id ? (instances[0]?.id ?? null) : s.selectedInstanceId,
      };
    });
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
