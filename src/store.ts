import { listen } from "@tauri-apps/api/event";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { create } from "zustand";
import {
  notifyInstalled,
  notifyRemoved,
  notifySummary,
  notifyTaskFinished,
} from "./lib/notify";

import { api } from "./lib/api";
import { emptyFilters, type FilterState } from "./components/FilterRail";
import { isInstanceInstalled } from "./lib/loader";
import { log } from "./lib/log";
import type {
  AccountView,
  ContentKind,
  ContentUpdate,
  PendingOperation,
  Task,
  Instance,
  InstalledItem,
  LauncherSettings,
  LogConfig,
  LogLevel,
  LogsTab,
  LogLine,
  LogRecord,
  RunningInfo,
  SearchPage,
  SearchProvider,
  SortOrder,
  VersionMedia,
  View,
} from "./lib/types";

const MAX_LOG_RECORDS = 5000;

export interface DiscoverBrowse {
  provider: SearchProvider;
  query: string;
  sort: SortOrder;
  filters: FilterState;
  offset: number;
  showFilters: boolean;
  scrollTop: number;
  page: SearchPage | null;
  signature: string | null;
  scope: string | null;
}

const emptyBrowse: DiscoverBrowse = {
  provider: "modrinth",
  query: "",
  sort: "relevance",
  filters: emptyFilters,
  offset: 0,
  showFilters: true,
  scrollTop: 0,
  page: null,
  signature: null,
  scope: null,
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

function pruneSupersededSessions(
  sessions: Record<string, RunningInfo>,
): Record<string, RunningInfo> {
  const liveInstances = new Set(
    Object.values(sessions)
      .filter((run) => run.state === "running")
      .map((run) => run.instance_id),
  );
  const newestFinished = new Map<string, RunningInfo>();
  for (const run of Object.values(sessions)) {
    if (run.state === "running" || liveInstances.has(run.instance_id)) continue;
    const newest = newestFinished.get(run.instance_id);
    if (!newest || run.started_at > newest.started_at) {
      newestFinished.set(run.instance_id, run);
    }
  }

  return Object.fromEntries(
    Object.entries(sessions).filter(
      ([, run]) =>
        run.state === "running" ||
        newestFinished.get(run.instance_id)?.running_id === run.running_id,
    ),
  );
}

function newestLiveSessionId(sessions: Record<string, RunningInfo>): string | null {
  return (
    Object.values(sessions)
      .filter((run) => run.state === "running")
      .sort((a, b) => b.started_at - a.started_at)[0]?.running_id ?? null
  );
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
  logRecords: LogRecord[];
  logConfig: LogConfig | null;
  skinRevision: number;
  skinHeads: Record<string, string>;
  activeRunningId: string | null;
  launching: string[];
  logsTab: LogsTab;
  media: Record<string, VersionMedia | null>;
  detailInstanceId: string | null;
  viewStack: View[];
  searchKind: ContentKind | null;
  discoverKind: ContentKind;
  discoverTargetId: string | null;
  discoverBrowse: DiscoverBrowse;
  projectRef: { provider: SearchProvider; id: string; title?: string } | null;
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
  setLogsTab: (tab: LogsTab) => void;
  bumpSkinRevision: () => void;
  setSkinHead: (uuid: string, dataUrl: string | null) => void;
  syncActiveSkin: () => Promise<void>;
  refreshLogs: () => Promise<void>;
  clearLogs: () => Promise<void>;
  setLogLevel: (level: LogLevel) => Promise<void>;
  loadMedia: (instanceId: string) => Promise<void>;
  selectInstance: (id: string) => void;
  openInstance: (id: string) => void;
  openSearch: (kind: ContentKind) => void;
  openProject: (
    provider: SearchProvider,
    id: string,
    kind?: ContentKind,
    title?: string,
  ) => void;
  openDiscover: (kind?: ContentKind, targetInstanceId?: string | null) => void;
  setDiscoverKind: (kind: ContentKind) => void;
  setDiscoverBrowse: (patch: Partial<DiscoverBrowse>) => void;
  resetDiscoverBrowse: (patch: Partial<DiscoverBrowse>) => void;
  setDiscoverTarget: (instanceId: string | null) => void;
  installModpack: (
    provider: SearchProvider,
    projectId: string,
    versionId: string,
  ) => Promise<Instance>;
  goBack: () => void;
  goBackTo: (index: number) => void;
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
  }) => Promise<InstalledItem[]>;
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
let initializing = false;
let batching = false;
let installsInFlight = 0;
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
  logRecords: [],
  logConfig: null,
  skinRevision: 0,
  skinHeads: {},
  activeRunningId: null,
  launching: [],
  logsTab: "launcher",
  media: {},
  selectedInstanceId: null,
  detailInstanceId: null,
  viewStack: [],
  searchKind: null,
  discoverKind: "mods",
  discoverTargetId: null,
  discoverBrowse: emptyBrowse,
  projectRef: null,
  contentSources: {},
  updates: {},
  interrupted: [],
  tasks: {},
  taskOrder: [],

  setView: (view) => set({ view, viewStack: [] }),

  goBack: () =>
    set((s) => ({
      view: s.viewStack[s.viewStack.length - 1] ?? "home",
      viewStack: s.viewStack.slice(0, -1),
    })),

  goBackTo: (index) =>
    set((s) => ({
      view: s.viewStack[index] ?? s.view,
      viewStack: s.viewStack.slice(0, index),
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

  setDiscoverBrowse: (patch) =>
    set((s) => ({ discoverBrowse: { ...s.discoverBrowse, ...patch } })),

  resetDiscoverBrowse: (patch) => set({ discoverBrowse: { ...emptyBrowse, ...patch } }),

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
    installsInFlight += 1;
    let items: InstalledItem[];
    try {
      items = await api.installContent(
        params.provider,
        params.projectId,
        params.instanceId,
        params.kind,
        params.gameVersion,
        params.loader,
        params.versionId ?? null,
        params.withDependencies ?? true,
      );
    } finally {
      installsInFlight -= 1;
    }
    const into =
      useStore.getState().instances.find((i) => i.id === params.instanceId)?.name ??
      "this instance";
    notifyInstalled(items, into);
    await useStore.getState().refreshContentSources(params.instanceId, params.kind);
    return items;
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

  openProject: (provider, id, kind, title) =>
    set((s) => ({
      projectRef: { provider, id, title },
      searchKind: kind ?? s.searchKind,
      view: "project",
      viewStack: s.view !== "project" ? [...s.viewStack.slice(-19), s.view] : s.viewStack,
    })),

  init: async () => {
    if (initializing || useStore.getState().ready) return;
    initializing = true;

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
          previous.state === "running" && task.state !== "running";

        const quiet = batching || (installsInFlight > 0 && task.kind === "content_install");
        if (justFinished && !quiet && task.state === "succeeded") {
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
      track(await listen<LogRecord[]>("log:record", (e) => {
        set((s) => {
          const last = s.logRecords[s.logRecords.length - 1]?.seq ?? 0;
          const fresh = e.payload.filter((r) => r.seq > last);
          if (fresh.length === 0) return {};
          const next = [...s.logRecords, ...fresh];
          if (next.length > MAX_LOG_RECORDS) {
            next.splice(0, next.length - MAX_LOG_RECORDS);
          }
          return { logRecords: next };
        });
      }));
      track(await listen<RunningInfo>("process:state", (e) => {
        const info = e.payload;
        set((s) => {
          const running = pruneSupersededSessions({
            ...s.running,
            [info.running_id]: info,
          });
          return {
            running,
            logs: Object.fromEntries(
              Object.entries(s.logs).filter(([runningId]) => runningId in running),
            ),
            activeRunningId:
              s.activeRunningId && s.activeRunningId in running
                ? s.activeRunningId
                : newestLiveSessionId(running),
          };
        });
        if (info.state !== "running") {
          void useStore.getState().refreshInstances();
        }
      }));
    }

    try {
      const [
        settings,
        instances,
        accounts,
        installedVersions,
        tasks,
        interrupted,
        recoveredRuns,
      ] = await Promise.all([
        api.getSettings(),
        api.listInstances(),
        api.listAccounts(),
        api.listInstalledVersions(),
        api.listTasks().catch(() => [] as Task[]),
        api.recoverInterrupted().catch(() => [] as PendingOperation[]),
        api.listRunning().catch(() => [] as RunningInfo[]),
      ]);
      const recoveredLogs = await Promise.all(
        recoveredRuns.map(async (run) => [
          run.running_id,
          await api.getLogs(run.running_id).catch(() => [] as LogLine[]),
        ] as const),
      );
      log.setLevel(settings.log_level);
      const installedIds = instances
        .filter((i) => isInstanceInstalled(i, installedVersions))
        .map((i) => i.id);
      set((s) => {
        const running = pruneSupersededSessions({
          ...Object.fromEntries(recoveredRuns.map((run) => [run.running_id, run])),
          ...s.running,
        });
        const logs = recoveredLogs.reduce(
          (logs, [runningId, backfill]) => {
            if (!(runningId in running)) return logs;
            const streamed = logs[runningId] ?? [];
            logs[runningId] = backfill.length >= streamed.length ? backfill : streamed;
            return logs;
          },
          Object.fromEntries(
            Object.entries(s.logs).filter(([runningId]) => runningId in running),
          ),
        );
        const activeRunningId =
          s.activeRunningId && s.activeRunningId in running
            ? s.activeRunningId
            : newestLiveSessionId(running);

        return {
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
          running,
          logs,
          activeRunningId,
        };
      });

      if (instances.some((i) => i.pack_project_id && !i.logo)) {
        api
          .backfillPackLogos()
          .then((updated) => {
            if (Array.isArray(updated)) set({ instances: updated });
          })
          .catch((e) => console.error("pack logo backfill failed:", e));
      }
      log.info("app", "launcher ready", {
        instances: instances.length,
        accounts: accounts.length,
      });
      void useStore.getState().refreshLogs();
      void useStore.getState().syncActiveSkin();
    } catch (e) {
      log.error("app", `startup failed: ${String(e)}`);
      set({ error: String(e), ready: true });
    } finally {
      initializing = false;
    }
  },

  bumpSkinRevision: () => set((s) => ({ skinRevision: s.skinRevision + 1 })),

  setSkinHead: (uuid, dataUrl) =>
    set((s) => {
      if (!dataUrl) {
        if (!(uuid in s.skinHeads)) return {};
        const next = { ...s.skinHeads };
        delete next[uuid];
        return { skinHeads: next };
      }
      if (s.skinHeads[uuid] === dataUrl) return {};
      return { skinHeads: { ...s.skinHeads, [uuid]: dataUrl } };
    }),

  syncActiveSkin: async () => {
    const active = useStore.getState().accounts.find((a) => a.active);
    if (!active) return;
    try {
      const worn = await api.getWornSkin(active.id);
      if (worn) useStore.getState().setSkinHead(active.id, worn.data_url);
    } catch (e) {
      log.debug("skins", `could not read the worn skin: ${String(e)}`);
    }
  },

  refreshLogs: async () => {
    const [records, config] = await Promise.all([
      api.getLogRecords(MAX_LOG_RECORDS),
      api.getLogConfig(),
    ]);
    log.setLevel(config.level);
    set((s) => {
      const highest = records[records.length - 1]?.seq ?? 0;
      const streamed = s.logRecords.filter((r) => r.seq > highest);
      return { logRecords: [...records, ...streamed], logConfig: config };
    });
  },

  clearLogs: async () => {
    await api.clearLogRecords();
    set({ logRecords: [] });
  },

  setLogLevel: async (level) => {
    const config = await api.setLogLevel(level);
    log.setLevel(config.level);
    set((s) => ({
      logConfig: config,
      settings: s.settings ? { ...s.settings, log_level: config.level } : s.settings,
    }));
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
    void useStore.getState().syncActiveSkin();
  },

  removeAccount: async (id) => {
    const name = useStore.getState().accounts.find((a) => a.id === id)?.name;
    await api.removeAccount(id);
    notifyRemoved(name ? `Signed out ${name}` : "Account removed");
    await useStore.getState().refreshAccounts();
  },

  resetAuth: () => set({ auth: { status: "idle" } }),

  launchInstance: async (id) => {
    if (useStore.getState().launching.includes(id)) return;
    set((s) => ({ launching: [...s.launching, id] }));
    let runningId: string;
    try {
      runningId = await api.launchInstance(id);
    } finally {
      set((s) => ({ launching: s.launching.filter((v) => v !== id) }));
    }
    set((s) => ({
      activeRunningId: runningId,
      logsTab: "game",
      view: "logs",
      viewStack: s.view !== "logs" ? [...s.viewStack.slice(-19), s.view] : s.viewStack,
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
      logsTab: "game",
      view: "logs",
      viewStack: s.view !== "logs" ? [...s.viewStack.slice(-19), s.view] : s.viewStack,
    })),

  setLogsTab: (tab) => set({ logsTab: tab }),

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
    const name = useStore.getState().instances.find((i) => i.id === id)?.name;
    await api.deleteInstance(id);
    notifyRemoved(name ? `Deleted ${name}` : "Instance deleted", "Its files are gone from disk");
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

const BROWSE_VIEWS = new Set<View>(["discover", "project"]);

useStore.subscribe((state, previous) => {
  if (state.view === previous.view) return;
  if (BROWSE_VIEWS.has(previous.view) && !BROWSE_VIEWS.has(state.view)) {
    state.resetDiscoverBrowse({});
  }
});

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
