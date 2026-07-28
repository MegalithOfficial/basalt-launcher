import { listen } from "@tauri-apps/api/event";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { create } from "zustand";
import { notifySummary, notifyTaskFinished } from "./lib/notify";

import { api } from "./lib/api";
import { isInstanceInstalled } from "./lib/loader";
import type {
  AccountView,
  ContentKind,
  ContentUpdate,
  PendingOperation,
  Task,
  Instance,
  LauncherSettings,
  LogLine,
  RunningInfo,
  SearchProvider,
  VersionMedia,
  View,
} from "./lib/types";

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
  discoverKind: ContentKind;
  discoverTargetId: string | null;
  projectRef: { provider: SearchProvider; id: string } | null;
  contentSources: Record<string, Record<string, { file_name: string; version_id: string | null }>>;
  updates: Record<string, ContentUpdate[]>;
  interrupted: PendingOperation[];
  tasks: Record<string, Task>;
  taskOrder: string[];
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
  openDiscover: (kind?: ContentKind, targetInstanceId?: string | null) => void;
  setDiscoverKind: (kind: ContentKind) => void;
  setDiscoverTarget: (instanceId: string | null) => void;
  installModpack: (
    provider: SearchProvider,
    projectId: string,
    versionId: string,
  ) => Promise<Instance>;
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
    withDependencies?: boolean;
  }) => Promise<string[]>;
  refreshUpdates: (instanceId: string, force?: boolean) => Promise<void>;
  clearFinishedTasks: () => Promise<void>;
  cancelTask: (taskId: string) => Promise<void>;
  dismissInterrupted: () => void;
  beginToastBatch: () => void;
  endToastBatch: (summary: string | null) => void;
  applyUpdate: (instanceId: string, kind: string, fileName: string) => Promise<void>;
  pickBanner: (instanceId: string) => Promise<void>;
  clearBanner: (instanceId: string) => Promise<void>;
  pickLogo: (instanceId: string) => Promise<void>;
  clearLogo: (instanceId: string) => Promise<void>;
}

let listenersBound = false;
let batching = false;
let unlisteners: Array<() => void> = [];

export const useStore = create<AppStore>((set) => ({
  view: "home",
  ready: false,
  error: null,
  settings: null,
  instances: [],
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
  discoverKind: "mods",
  discoverTargetId: null,
  projectRef: null,
  contentSources: {},
  updates: {},
  interrupted: [],
  tasks: {},
  taskOrder: [],

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
      discoverKind: kind,
      discoverTargetId: s.detailInstanceId,
      view: "discover",
      viewStack: s.view !== "discover" ? [...s.viewStack.slice(-19), s.view] : s.viewStack,
    })),

  openDiscover: (kind, targetInstanceId) =>
    set((s) => ({
      discoverKind: kind ?? s.discoverKind,
      searchKind: kind ?? s.discoverKind,
      discoverTargetId: targetInstanceId !== undefined ? targetInstanceId : s.discoverTargetId,
      view: "discover",
      viewStack: s.view !== "discover" ? [...s.viewStack.slice(-19), s.view] : s.viewStack,
    })),

  setDiscoverKind: (kind) => set({ discoverKind: kind, searchKind: kind }),

  setDiscoverTarget: (instanceId) => set({ discoverTargetId: instanceId }),

  installModpack: async (provider, projectId, versionId) => {
    const instance = await api.installModpack(provider, projectId, versionId);
    const installedVersions = await api.listInstalledVersions();
    set((s) => {
      const instances = s.instances.some((i) => i.id === instance.id)
        ? s.instances.map((i) => (i.id === instance.id ? instance : i))
        : [...s.instances, instance];
      return {
        instances,
        selectedInstanceId: instance.id,
        installedIds: instances
          .filter((i) => isInstanceInstalled(i, installedVersions))
          .map((i) => i.id),
      };
    });
    return instance;
  },

  refreshContentSources: async (instanceId, kind) => {
    try {
      const items = await api.listInstanceContent(instanceId, kind);
      const map: Record<string, { file_name: string; version_id: string | null }> = {};
      items.forEach((item) => {
        if (item.source?.project_id) {
          map[item.source.project_id] = {
            file_name: item.file_name,
            version_id: item.source.version_id,
          };
        }
      });
      set((s) => ({
        contentSources: { ...s.contentSources, [`${instanceId}:${kind}`]: map },
      }));
    } catch {
      return;
    }
  },

  installContent: async (params) => {
    const files = await api.installContent(
      params.provider,
      params.projectId,
      params.instanceId,
      params.kind,
      params.gameVersion,
      params.loader,
      params.versionId ?? null,
      params.withDependencies ?? true,
    );
    await useStore.getState().refreshContentSources(params.instanceId, params.kind);
    return files;
  },

  cancelTask: async (taskId) => {
    await api.cancelTask(taskId);
  },

  dismissInterrupted: () => set({ interrupted: [] }),

  beginToastBatch: () => {
    batching = true;
  },

  endToastBatch: (summary) => {
    batching = false;
    if (summary) notifySummary(summary);
  },

  clearFinishedTasks: async () => {
    await api.clearFinishedTasks();
    const remaining = await api.listTasks();
    set({
      tasks: Object.fromEntries(remaining.map((t) => [t.id, t])),
      taskOrder: remaining.map((t) => t.id),
    });
  },

  refreshUpdates: async (instanceId, force = false) => {
    try {
      const list = force
        ? await api.checkContentUpdates(instanceId, true)
        : await api.getContentUpdates(instanceId);
      set((s) => ({ updates: { ...s.updates, [instanceId]: list } }));
      if (!force) {
        const fresh = await api.checkContentUpdates(instanceId, false);
        set((s) => ({ updates: { ...s.updates, [instanceId]: fresh } }));
      }
    } catch {
      return;
    }
  },

  applyUpdate: async (instanceId, kind, fileName) => {
    await api.applyContentUpdate(instanceId, kind, fileName);
    set((s) => ({
      updates: {
        ...s.updates,
        [instanceId]: (s.updates[instanceId] ?? []).filter(
          (u) => !(u.kind === kind && u.file_name === fileName),
        ),
      },
    }));
    await useStore.getState().refreshContentSources(instanceId, kind);
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
      const track = (fn: () => void) => unlisteners.push(fn);
      track(await listen<AuthPayload>("auth:state", (e) => {
        const p = e.payload;
        if (p.status === "success") {
          set({ auth: { status: "idle" } });
          void useStore.getState().refreshAccounts();
        } else {
          set({ auth: { status: "error", message: p.message } });
        }
      }));
      track(await listen<Task>("task:update", (e) => {
        const task = e.payload;
        const previous = useStore.getState().tasks[task.id];
        const justFinished =
          previous &&
          (previous.state === "running" || previous.state === "queued") &&
          task.state !== "running" &&
          task.state !== "queued";

        if (justFinished && !batching && task.state === "succeeded") {
          notifyTaskFinished(task);
        }
        if (justFinished && task.state === "failed") {
          notifyTaskFinished(task);
        }

        const known = useStore.getState().instances;
        if (task.instance_id && !known.some((i) => i.id === task.instance_id)) {
          void useStore.getState().refreshInstances();
        }
        if (task.state === "cancelled" || task.state === "failed") {
          void useStore.getState().refreshInstances();
        }
        set((s) => {
          const marksInstalled =
            task.state === "succeeded" &&
            (task.kind === "game_install" || task.kind === "modpack_install") &&
            !!task.instance_id;
          return {
            tasks: { ...s.tasks, [task.id]: task },
            taskOrder: s.taskOrder.includes(task.id)
              ? s.taskOrder
              : [...s.taskOrder, task.id],
            installedIds:
              marksInstalled && !s.installedIds.includes(task.instance_id!)
                ? [...s.installedIds, task.instance_id!]
                : s.installedIds,
          };
        });
      }));
      track(await listen<LogPayload>("process:log", (e) => {
        const p = e.payload;
        set((s) => {
          const prev = s.logs[p.running_id] ?? [];
          const next = [...prev, { stream: p.stream, line: p.line }];
          if (next.length > 6000) next.splice(0, next.length - 6000);
          return { logs: { ...s.logs, [p.running_id]: next } };
        });
      }));
      track(await listen<RunningInfo>("process:state", (e) => {
        const info = e.payload;
        set((s) => ({ running: { ...s.running, [info.running_id]: info } }));
        if (info.state !== "running") {
          void useStore.getState().refreshInstances();
        }
      }));
    }

    try {
      const [settings, instances, accounts, installedVersions, tasks, interrupted] = await Promise.all([
        api.getSettings(),
        api.listInstances(),
        api.listAccounts(),
        api.listInstalledVersions(),
        api.listTasks().catch(() => [] as Task[]),
        api.recoverInterrupted().catch(() => [] as PendingOperation[]),
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
        discoverTargetId: s.discoverTargetId ?? instances[0]?.id ?? null,
        tasks: Object.fromEntries(tasks.map((t) => [t.id, t])),
        taskOrder: tasks.map((t) => t.id),
        interrupted,
      }));

      if (instances.some((i) => i.pack_project_id && !i.logo)) {
        api
          .backfillPackLogos()
          .then((updated) => {
            if (Array.isArray(updated)) set({ instances: updated });
          })
          .catch((e) => console.error("pack logo backfill failed:", e));
      }
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

  pickLogo: async (instanceId) => {
    const file = await openFileDialog({
      multiple: false,
      directory: false,
      filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "webp", "gif"] }],
    });
    if (typeof file !== "string") return;
    const logo = await api.setInstanceLogo(instanceId, file);
    set((s) => ({
      instances: s.instances.map((i) => (i.id === instanceId ? { ...i, logo } : i)),
    }));
  },

  clearLogo: async (instanceId) => {
    await api.clearInstanceLogo(instanceId);
    set((s) => ({
      instances: s.instances.map((i) => (i.id === instanceId ? { ...i, logo: null } : i)),
    }));
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
      discoverTargetId: s.discoverTargetId ?? instance.id,
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
    try {
      await api.installInstance(id);
    } catch (e) {
      set({ error: String(e) });
    }
  },
}));

if (import.meta.env.DEV) {
  (window as unknown as { __store: typeof useStore }).__store = useStore;
}

if (import.meta.hot) {
  import.meta.hot.dispose(() => {
    unlisteners.forEach((fn) => fn());
    unlisteners = [];
    listenersBound = false;
  });
}
