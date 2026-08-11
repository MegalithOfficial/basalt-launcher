import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { toast } from "sonner";
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
  AppUpdateStatus,
  ConsoleLine,
  ContentKind,
  ContentUpdate,
  PendingOperation,
  Task,
  Instance,
  InstanceGroup,
  InstanceOrganization,
  InstalledItem,
  LauncherSettings,
  LogConfig,
  LogLevel,
  LogsTab,
  LogLine,
  LogRecord,
  ManualDownloadSource,
  RunningInfo,
  RepairReport,
  Server,
  ServerFlavor,
  ServerSoftware,
  ServerRunningInfo,
  ServerUsage,
  SearchPage,
  SearchProvider,
  SortOrder,
  TaskKind,
  VersionMedia,
  View,
} from "./lib/types";

const MAX_LOG_RECORDS = 5000;
const MAX_GAME_LOG_LINES = 6000;
const PROCESS_LOG_FLUSH_MS = 80;
const MAX_CONSOLE_LINES = 6000;
const MAX_USAGE_SAMPLES = 120;

const UNANNOUNCED = new Set<TaskKind>([
  "app_update",
  "instance_repair",
  "snapshot_create",
  "snapshot_restore",
  "storage_scan",
]);

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
  seededFor: string | null;
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
  seededFor: null,
};

interface AuthPayload {
  status: "success" | "error";
  account?: AccountView;
  message?: string;
}

interface LogPayload {
  running_id: string;
  stream: string;
  lines: string[];
}

interface ConsolePayload {
  server_id: string;
  stream: string;
  lines: string[];
}

interface OfflineLaunchPayload {
  instance_id: string;
  account_name: string;
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
  bundledCurseforgeKey: boolean;
  instances: Instance[];
  instanceOrganization: InstanceOrganization;
  installedIds: string[];
  accounts: AccountView[];
  auth: AuthFlow;
  running: Record<string, RunningInfo>;
  logs: Record<string, LogLine[]>;
  servers: Server[];
  serverSoftware: ServerSoftware[];
  serverRunning: Record<string, ServerRunningInfo>;
  serverConsole: Record<string, ConsoleLine[]>;
  serverUsage: Record<string, ServerUsage[]>;
  logRecords: LogRecord[];
  logConfig: LogConfig | null;
  skinRevision: number;
  skinHeads: Record<string, string>;
  activeRunningId: string | null;
  launching: string[];
  logsTab: LogsTab;
  media: Record<string, VersionMedia | null>;
  detailInstanceId: string | null;
  detailServerId: string | null;
  viewStack: View[];
  searchKind: ContentKind | null;
  discoverKind: ContentKind;
  discoverTargetId: string | null;
  discoverWorld: string | null;
  discoverBrowse: DiscoverBrowse;
  projectRef: { provider: SearchProvider; id: string; title?: string } | null;
  contentSources: Record<string, Record<string, { file_name: string; version_id: string | null }>>;
  updates: Record<string, ContentUpdate[]>;
  interrupted: PendingOperation[];
  tasks: Record<string, Task>;
  taskOrder: string[];
  appUpdateStatus: AppUpdateStatus | null;
  selectedInstanceId: string | null;

  setView: (view: View) => void;
  creatingInstance: boolean;
  startInstanceCreate: () => void;
  endInstanceCreate: () => void;
  init: () => Promise<void>;
  refreshInstances: () => Promise<void>;
  refreshServers: () => Promise<void>;
  openServer: (id: string) => void;
  createServer: (
    name: string,
    flavor: ServerFlavor,
    versionId: string,
    flavorVersion: string | null,
  ) => Promise<Server>;
  importServer: (
    path: string,
    name: string,
    flavor: ServerFlavor,
    versionId: string,
    flavorVersion: string | null,
  ) => Promise<Server>;
  installServer: (serverId: string) => Promise<void>;
  startServer: (serverId: string) => Promise<void>;
  stopServer: (serverId: string) => Promise<void>;
  restartServer: (serverId: string) => Promise<void>;
  forceStopServer: (serverId: string) => Promise<void>;
  sendServerCommand: (serverId: string, line: string) => Promise<void>;
  deleteServer: (serverId: string, deleteFiles: boolean) => Promise<void>;
  refreshInstanceOrganization: () => Promise<void>;
  createInstanceGroup: (name: string) => Promise<InstanceGroup>;
  renameInstanceGroup: (id: string, name: string) => Promise<void>;
  deleteInstanceGroup: (id: string) => Promise<void>;
  moveInstanceToGroup: (instanceId: string, groupId: string | null) => Promise<void>;
  reorderInstanceGroups: (ids: string[]) => Promise<void>;
  reorderGroupInstances: (groupId: string | null, ids: string[]) => Promise<void>;
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
    jvmArgs?: string | null,
    jvmArgsMode?: string | null,
    envVars?: string | null,
    envVarsMode?: string | null,
  ) => Promise<void>;
  deleteInstance: (id: string) => Promise<void>;
  deleteInstances: (ids: string[]) => Promise<string[]>;
  pins: string[];
  togglePin: (id: string) => void;
  repairInstance: (id: string) => Promise<RepairReport>;
  duplicateInstance: (id: string) => Promise<Instance>;
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
  openDiscover: (
    kind?: ContentKind,
    targetInstanceId?: string | null,
    world?: string | null,
  ) => void;
  setDiscoverKind: (kind: ContentKind) => void;
  setDiscoverBrowse: (patch: Partial<DiscoverBrowse>) => void;
  resetDiscoverBrowse: (patch: Partial<DiscoverBrowse>) => void;
  setDiscoverTarget: (instanceId: string | null) => void;
  setDiscoverWorld: (world: string | null) => void;
  installModpack: (
    provider: SearchProvider,
    projectId: string,
    versionId: string,
    manualDownloads?: ManualDownloadSource[],
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
  dismissAppUpdate: (version: string) => Promise<void>;
  downloadAppUpdate: () => Promise<void>;
  installAppUpdate: () => Promise<void>;
  beginOptimisticTask: (
    kind: Task["kind"],
    title: string,
    context?: {
      subtitle?: string | null;
      iconUrl?: string | null;
      instanceId?: string | null;
      projectId?: string | null;
    },
  ) => string;
  endOptimisticTask: (taskId: string) => void;
  dismissInterrupted: () => void;
  beginToastBatch: () => void;
  endToastBatch: (summary: string | null) => void;
  applyUpdate: (
    instanceId: string,
    kind: string,
    fileName: string,
    manualDownloads?: ManualDownloadSource[],
  ) => Promise<void>;
  pickBanner: (instanceId: string) => Promise<void>;
  clearBanner: (instanceId: string) => Promise<void>;
  pickLogo: (instanceId: string) => Promise<void>;
  applyBanner: (instanceId: string, bannerId: string) => Promise<void>;
  applyLogo: (instanceId: string, bannerId: string) => Promise<void>;
  clearLogo: (instanceId: string) => Promise<void>;
}

let listenersBound = false;
let initializing = false;
let batching = false;
let installsInFlight = 0;
let unlisteners: Array<() => void> = [];
let processLogTimer: ReturnType<typeof setTimeout> | null = null;
let minimizedForPlay = false;

function restoreAfterPlay() {
  if (!minimizedForPlay) return;
  const stillPlaying = Object.values(useStore.getState().running).some(
    (session) => session.state === "running",
  );
  if (stillPlaying) return;
  minimizedForPlay = false;
  const win = getCurrentWindow();
  void win
    .unminimize()
    .then(() => win.setFocus())
    .catch(() => {});
}
const pendingProcessLogs = new Map<string, LogLine[]>();

function flushProcessLogs() {
  processLogTimer = null;
  if (pendingProcessLogs.size === 0) return;

  const batches = new Map(pendingProcessLogs);
  pendingProcessLogs.clear();
  useStore.setState((state) => {
    const logs = { ...state.logs };
    for (const [runningId, incoming] of batches) {
      const previous = logs[runningId] ?? [];
      const keepPrevious = Math.max(0, MAX_GAME_LOG_LINES - incoming.length);
      logs[runningId] = [...previous.slice(-keepPrevious), ...incoming.slice(-MAX_GAME_LOG_LINES)];
    }
    return { logs };
  });
}

function enqueueProcessLog(payload: LogPayload) {
  const pending = pendingProcessLogs.get(payload.running_id) ?? [];
  pending.push(...payload.lines.map((line) => ({ stream: payload.stream, line })));
  if (pending.length > MAX_GAME_LOG_LINES) {
    pending.splice(0, pending.length - MAX_GAME_LOG_LINES);
  }
  pendingProcessLogs.set(payload.running_id, pending);
  processLogTimer ??= setTimeout(flushProcessLogs, PROCESS_LOG_FLUSH_MS);
}

const pendingConsole = new Map<string, ConsoleLine[]>();
let consoleTimer: ReturnType<typeof setTimeout> | null = null;

function flushConsole() {
  consoleTimer = null;
  if (pendingConsole.size === 0) return;

  const batches = new Map(pendingConsole);
  pendingConsole.clear();
  useStore.setState((state) => {
    const serverConsole = { ...state.serverConsole };
    for (const [serverId, incoming] of batches) {
      const previous = serverConsole[serverId] ?? [];
      const keepPrevious = Math.max(0, MAX_CONSOLE_LINES - incoming.length);
      serverConsole[serverId] = [
        ...previous.slice(-keepPrevious),
        ...incoming.slice(-MAX_CONSOLE_LINES),
      ];
    }
    return { serverConsole };
  });
}

function enqueueConsole(payload: ConsolePayload) {
  const pending = pendingConsole.get(payload.server_id) ?? [];
  pending.push(...payload.lines.map((line) => ({ stream: payload.stream, line })));
  if (pending.length > MAX_CONSOLE_LINES) {
    pending.splice(0, pending.length - MAX_CONSOLE_LINES);
  }
  pendingConsole.set(payload.server_id, pending);
  consoleTimer ??= setTimeout(flushConsole, PROCESS_LOG_FLUSH_MS);
}

function pushStack(stack: View[], current: View, target: View): View[] {
  if (current === target) return stack;
  const seen = stack.indexOf(target);
  if (seen >= 0) return stack.slice(0, seen);
  return [...stack.filter((view) => view !== current), current].slice(-3);
}

function pruneInstances(ids: string[]) {
  const gone = new Set(ids);
  return (s: AppStore): Partial<AppStore> => {
    const instances = s.instances.filter((i) => !gone.has(i.id));
    const closingDetail = !!s.detailInstanceId && gone.has(s.detailInstanceId);
    const media = { ...s.media };
    const updates = { ...s.updates };
    for (const id of gone) {
      delete media[id];
      delete updates[id];
    }
    const contentSources = Object.fromEntries(
      Object.entries(s.contentSources).filter(
        ([key]) => ![...gone].some((id) => key.startsWith(`${id}:`)),
      ),
    );
    return {
      instances,
      instanceOrganization: {
        ...s.instanceOrganization,
        placements: s.instanceOrganization.placements.filter(
          (placement) => !gone.has(placement.instance_id),
        ),
      },
      view: closingDetail && s.view === "instance" ? "instances" : s.view,
      viewStack: closingDetail
        ? s.viewStack.filter((view) => view !== "instance")
        : s.viewStack,
      detailInstanceId: closingDetail ? null : s.detailInstanceId,
      discoverTargetId:
        s.discoverTargetId && gone.has(s.discoverTargetId) ? null : s.discoverTargetId,
      media,
      updates,
      contentSources,
      installedIds: s.installedIds.filter((x) => !gone.has(x)),
      selectedInstanceId:
        s.selectedInstanceId && gone.has(s.selectedInstanceId)
          ? (instances[0]?.id ?? null)
          : s.selectedInstanceId,
    };
  };
}

export const useStore = create<AppStore>((set) => ({
  view: "home",
  ready: false,
  error: null,
  settings: null,
  bundledCurseforgeKey: false,
  instances: [],
  instanceOrganization: { groups: [], placements: [] },
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
  servers: [],
  serverSoftware: [],
  serverRunning: {},
  serverConsole: {},
  serverUsage: {},
  launching: [],
  logsTab: "launcher",
  media: {},
  selectedInstanceId: null,
  detailInstanceId: null,
  detailServerId: null,
  viewStack: [],
  searchKind: null,
  discoverKind: "mods",
  discoverTargetId: null,
  discoverWorld: null,
  discoverBrowse: emptyBrowse,
  projectRef: null,
  contentSources: {},
  updates: {},
  interrupted: [],
  tasks: {},
  taskOrder: [],
  appUpdateStatus: null,

  setView: (view) => set({ view, viewStack: [] }),

  creatingInstance: false,
  startInstanceCreate: () =>
    set({ view: "instances", viewStack: [], creatingInstance: true }),
  endInstanceCreate: () => set({ creatingInstance: false }),

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
      viewStack: pushStack(s.viewStack, s.view, "discover"),
    })),

  openDiscover: (kind, targetInstanceId, world) =>
    set((s) => ({
      discoverKind: kind ?? s.discoverKind,
      searchKind: kind ?? s.discoverKind,
      discoverTargetId: targetInstanceId !== undefined ? targetInstanceId : s.discoverTargetId,
      discoverWorld: world !== undefined ? world : s.discoverWorld,
      view: "discover",
      viewStack: pushStack(s.viewStack, s.view, "discover"),
    })),

  setDiscoverKind: (kind) => set({ discoverKind: kind, searchKind: kind }),

  setDiscoverBrowse: (patch) =>
    set((s) => ({ discoverBrowse: { ...s.discoverBrowse, ...patch } })),

  resetDiscoverBrowse: (patch) => set({ discoverBrowse: { ...emptyBrowse, ...patch } }),

  setDiscoverTarget: (instanceId) => set({ discoverTargetId: instanceId, discoverWorld: null }),

  setDiscoverWorld: (world) => set({ discoverWorld: world }),

  installModpack: async (provider, projectId, versionId, manualDownloads = []) => {
    const instance = await api.installModpack(
      provider,
      projectId,
      versionId,
      manualDownloads,
    );
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
    await useStore.getState().refreshInstanceOrganization();
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

  dismissAppUpdate: async (version) => {
    const appUpdateStatus = await api.dismissAppUpdate(version);
    set({ appUpdateStatus });
  },

  downloadAppUpdate: async () => {
    const appUpdateStatus = await api.downloadAppUpdate();
    set({ appUpdateStatus });
  },

  installAppUpdate: async () => {
    await api.installAppUpdate();
  },

  beginOptimisticTask: (kind, title, context = {}) => {
    const id = `optimistic:${crypto.randomUUID()}`;
    const task: Task = {
      id,
      kind,
      title,
      subtitle: context.subtitle ?? null,
      icon_url: context.iconUrl ?? null,
      instance_id: context.instanceId ?? null,
      server_id: null,
      project_id: context.projectId ?? null,
      state: "running",
      stage: "starting",
      completed: 0,
      total: 0,
      downloaded_bytes: 0,
      total_bytes: 0,
      error: null,
      retries: 0,
      retry_note: null,
      started_at: Math.floor(Date.now() / 1000),
      finished_at: null,
    };
    set((state) => ({
      tasks: { ...state.tasks, [id]: task },
      taskOrder: [...state.taskOrder, id],
    }));
    return id;
  },

  endOptimisticTask: (taskId) => {
    set((state) => {
      if (!(taskId in state.tasks)) return {};
      const tasks = { ...state.tasks };
      delete tasks[taskId];
      return {
        tasks,
        taskOrder: state.taskOrder.filter((id) => id !== taskId),
      };
    });
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
    set((state) => {
      const optimistic = Object.values(state.tasks).filter((task) =>
        task.id.startsWith("optimistic:"),
      );
      const tasks = [...optimistic, ...remaining];
      return {
        tasks: Object.fromEntries(tasks.map((task) => [task.id, task])),
        taskOrder: tasks.map((task) => task.id),
      };
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

  applyUpdate: async (instanceId, kind, fileName, manualDownloads = []) => {
    await api.applyContentUpdate(instanceId, kind, fileName, manualDownloads);
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
      viewStack: pushStack(s.viewStack, s.view, "project"),
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
      track(await listen<AppUpdateStatus>("app:update-status", (e) => {
        set({ appUpdateStatus: e.payload });
      }));
      track(await listen<OfflineLaunchPayload>("launch:offline", (e) => {
        const instance = useStore
          .getState()
          .instances.find((candidate) => candidate.id === e.payload.instance_id);
        toast.warning("Started in offline mode", {
          description: `Microsoft services could not be reached. ${instance?.name ?? "Minecraft"} is using the cached ${e.payload.account_name} profile; online servers and account services may be unavailable.`,
          duration: 8_000,
        });
      }));
      track(await listen("data:relocated", () => {
        void useStore.getState().refreshInstances();
      }));
      track(await listen<Task>("task:update", (e) => {
        const task = e.payload;
        const previous = useStore.getState().tasks[task.id];
        const justFinished =
          previous &&
          previous.state === "running" && task.state !== "running";

        const quiet = batching || (installsInFlight > 0 && task.kind === "content_install");
        if (
          justFinished &&
          !quiet &&
          !UNANNOUNCED.has(task.kind) &&
          task.state === "succeeded"
        ) {
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
          const superseded = Object.values(s.tasks)
            .filter((candidate) => candidate.id.startsWith("optimistic:"))
            .filter(
              (candidate) =>
                (!!task.project_id && candidate.project_id === task.project_id) ||
                (!!task.instance_id &&
                  candidate.instance_id === task.instance_id &&
                  candidate.kind === task.kind),
            );
          const iconUrl =
            task.icon_url ??
            s.tasks[task.id]?.icon_url ??
            superseded.find((candidate) => candidate.icon_url)?.icon_url ??
            null;
          const supersededIds = superseded.map((candidate) => candidate.id);
          const tasks = { ...s.tasks };
          for (const id of supersededIds) delete tasks[id];
          const taskOrder = s.taskOrder.filter((id) => !supersededIds.includes(id));
          if (!taskOrder.includes(task.id)) taskOrder.push(task.id);
          const marksInstalled =
            task.state === "succeeded" &&
            (task.kind === "game_install" || task.kind === "modpack_install") &&
            !!task.instance_id;
          return {
            tasks: { ...tasks, [task.id]: { ...task, icon_url: iconUrl } },
            taskOrder,
            installedIds:
              marksInstalled && !s.installedIds.includes(task.instance_id!)
                ? [...s.installedIds, task.instance_id!]
                : s.installedIds,
          };
        });
      }));
      track(await listen<LogPayload>("process:log", (e) => {
        enqueueProcessLog(e.payload);
      }));
      track(await listen<ConsolePayload>("server:log", (e) => {
        enqueueConsole(e.payload);
      }));
      track(await listen<ServerRunningInfo>("server:state", (e) => {
        const info = e.payload;
        set((s) => ({ serverRunning: { ...s.serverRunning, [info.server_id]: info } }));
        if (info.state !== "running" && info.state !== "stopping") {
          void useStore.getState().refreshServers();
        }
      }));
      track(await listen<ServerUsage[]>("server:usage", (e) => {
        set((s) => {
          const serverUsage = { ...s.serverUsage };
          for (const sample of e.payload) {
            const previous = serverUsage[sample.server_id] ?? [];
            serverUsage[sample.server_id] = [...previous, sample].slice(-MAX_USAGE_SAMPLES);
          }
          return { serverUsage };
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
          restoreAfterPlay();
        }
      }));
    }

    try {
      const [
        settings,
        instances,
        instanceOrganization,
        accounts,
        installedVersions,
        tasks,
        interrupted,
        recoveredRuns,
        appUpdateStatus,
        servers,
        runningServers,
      ] = await Promise.all([
        api.getSettings(),
        api.listInstances(),
        api.getInstanceOrganization(),
        api.listAccounts(),
        api.listInstalledVersions(),
        api.listTasks().catch(() => [] as Task[]),
        api.recoverInterrupted().catch(() => [] as PendingOperation[]),
        api.listRunning().catch(() => [] as RunningInfo[]),
        api.getAppUpdateStatus().catch(() => null),
        api.listServers().catch(() => [] as Server[]),
        api.listRunningServers().catch(() => [] as ServerRunningInfo[]),
      ]);
      const bundledCurseforgeKey = await api
        .getAppInfo()
        .then((info) => info.bundled_curseforge_key)
        .catch(() => false);
      const recoveredConsoles = await Promise.all(
        runningServers.map(async (info) => [
          info.server_id,
          await api.getServerConsole(info.server_id).catch(() => [] as ConsoleLine[]),
        ] as const),
      );
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

        const serverConsole = recoveredConsoles.reduce(
          (consoles, [serverId, backfill]) => {
            const streamed = consoles[serverId] ?? [];
            consoles[serverId] = backfill.length >= streamed.length ? backfill : streamed;
            return consoles;
          },
          { ...s.serverConsole } as Record<string, ConsoleLine[]>,
        );

        return {
          settings,
          bundledCurseforgeKey,
          instances,
          instanceOrganization,
          accounts,
          installedIds,
          servers,
          serverRunning: Object.fromEntries(
            runningServers.map((info) => [info.server_id, info]),
          ),
          serverConsole,
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
          appUpdateStatus,
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
      viewStack: pushStack(s.viewStack, s.view, "logs"),
    }));
    if (useStore.getState().settings?.minimize_on_launch) {
      minimizedForPlay = true;
      void getCurrentWindow().minimize().catch(() => {});
    }
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
    pendingProcessLogs.delete(runningId);
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
      viewStack: pushStack(s.viewStack, s.view, "logs"),
    })),

  setLogsTab: (tab) => set({ logsTab: tab }),

  selectInstance: (id) => set({ selectedInstanceId: id }),

  openInstance: (id) =>
    set((s) => ({
      detailInstanceId: id,
      view: "instance",
      viewStack: s.view === "instance" ? s.viewStack : ["instances"],
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

  applyBanner: async (instanceId, bannerId) => {
    const media = await api.applyBanner(instanceId, bannerId);
    set((s) => ({ media: { ...s.media, [instanceId]: media } }));
  },

  applyLogo: async (instanceId, bannerId) => {
    const logo = await api.applyLogo(instanceId, bannerId);
    set((s) => ({
      instances: s.instances.map((i) => (i.id === instanceId ? { ...i, logo } : i)),
    }));
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

  openServer: (id) =>
    set((s) => ({
      detailServerId: id,
      view: "server",
      viewStack: pushStack(s.viewStack, s.view, "server"),
    })),

  refreshServers: async () => {
    const known = useStore.getState().serverSoftware;
    const [servers, serverSoftware] = await Promise.all([
      api.listServers(),
      known.length ? known : api.listServerSoftware(),
    ]);
    set({ servers, serverSoftware });
  },

  createServer: async (name, flavor, versionId, flavorVersion) => {
    const server = await api.createServer(name, flavor, versionId, flavorVersion, true);
    set((s) => ({ servers: [...s.servers, server] }));
    return server;
  },

  importServer: async (path, name, flavor, versionId, flavorVersion) => {
    const server = await api.importServer(path, name, flavor, versionId, flavorVersion, true);
    set((s) => ({ servers: [...s.servers, server] }));
    return server;
  },

  installServer: async (serverId) => {
    const server = await api.installServer(serverId);
    set((s) => ({ servers: s.servers.map((x) => (x.id === server.id ? server : x)) }));
  },

  startServer: async (serverId) => {
    const info = await api.startServer(serverId);
    set((s) => ({
      serverConsole: { ...s.serverConsole, [serverId]: [] },
      serverUsage: { ...s.serverUsage, [serverId]: [] },
      serverRunning: { ...s.serverRunning, [serverId]: info },
    }));
    await useStore.getState().refreshServers();
  },

  stopServer: async (serverId) => {
    await api.stopServer(serverId);
  },

  restartServer: async (serverId) => {
    const info = await api.restartServer(serverId);
    set((s) => ({ serverRunning: { ...s.serverRunning, [serverId]: info } }));
  },

  forceStopServer: async (serverId) => {
    await api.forceStopServer(serverId);
  },

  sendServerCommand: async (serverId, line) => {
    set((s) => ({
      serverConsole: {
        ...s.serverConsole,
        [serverId]: [...(s.serverConsole[serverId] ?? []), { stream: "input", line }],
      },
    }));
    await api.sendServerCommand(serverId, line);
  },

  deleteServer: async (serverId, deleteFiles) => {
    await api.deleteServer(serverId, deleteFiles);
    set((s) => {
      const serverRunning = { ...s.serverRunning };
      const serverConsole = { ...s.serverConsole };
      const serverUsage = { ...s.serverUsage };
      delete serverRunning[serverId];
      delete serverConsole[serverId];
      delete serverUsage[serverId];
      const closingDetail = s.detailServerId === serverId;
      return {
        servers: s.servers.filter((server) => server.id !== serverId),
        serverRunning,
        serverConsole,
        serverUsage,
        detailServerId: closingDetail ? null : s.detailServerId,
        view: closingDetail && s.view === "server" ? "servers" : s.view,
        viewStack: closingDetail
          ? s.viewStack.filter((view) => view !== "server")
          : s.viewStack,
      };
    });
  },

  refreshInstances: async () => {
    const [instances, instanceOrganization] = await Promise.all([
      api.listInstances(),
      api.getInstanceOrganization(),
    ]);
    set({ instances, instanceOrganization });
  },

  refreshInstanceOrganization: async () => {
    set({ instanceOrganization: await api.getInstanceOrganization() });
  },

  createInstanceGroup: async (name) => {
    const group = await api.createInstanceGroup(name);
    set((state) => ({
      instanceOrganization: {
        ...state.instanceOrganization,
        groups: [...state.instanceOrganization.groups, group],
      },
    }));
    return group;
  },

  renameInstanceGroup: async (id, name) => {
    const group = await api.renameInstanceGroup(id, name);
    set((state) => ({
      instanceOrganization: {
        ...state.instanceOrganization,
        groups: state.instanceOrganization.groups.map((item) =>
          item.id === id ? group : item,
        ),
      },
    }));
  },

  deleteInstanceGroup: async (id) => {
    await api.deleteInstanceGroup(id);
    set((state) => ({
      instanceOrganization: {
        groups: state.instanceOrganization.groups.filter((group) => group.id !== id),
        placements: state.instanceOrganization.placements.map((placement) =>
          placement.group_id === id ? { ...placement, group_id: null } : placement,
        ),
      },
    }));
  },

  moveInstanceToGroup: async (instanceId, groupId) => {
    await api.moveInstanceToGroup(instanceId, groupId);
    await useStore.getState().refreshInstanceOrganization();
  },

  reorderInstanceGroups: async (ids) => {
    await api.reorderInstanceGroups(ids);
    set((state) => {
      const order = new Map(ids.map((id, index) => [id, index]));
      return {
        instanceOrganization: {
          ...state.instanceOrganization,
          groups: state.instanceOrganization.groups
            .map((group) => ({ ...group, sort_order: order.get(group.id) ?? group.sort_order }))
            .sort((a, b) => a.sort_order - b.sort_order),
        },
      };
    });
  },

  reorderGroupInstances: async (groupId, ids) => {
    await api.reorderGroupInstances(groupId, ids);
    set((state) => {
      const order = new Map(ids.map((id, index) => [id, index]));
      return {
        instanceOrganization: {
          ...state.instanceOrganization,
          placements: state.instanceOrganization.placements.map((placement) =>
            placement.group_id === groupId
              ? { ...placement, sort_order: order.get(placement.instance_id) ?? placement.sort_order }
              : placement,
          ),
        },
      };
    });
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
    await useStore.getState().refreshInstanceOrganization();
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
    jvmArgs = null,
    jvmArgsMode = null,
    envVars = null,
    envVarsMode = null,
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
      jvmArgs,
      jvmArgsMode,
      envVars,
      envVarsMode,
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

  pins: (() => {
    try {
      const raw = localStorage.getItem("sidebar-pins");
      const parsed = raw ? JSON.parse(raw) : [];
      return Array.isArray(parsed) ? parsed.filter((v) => typeof v === "string") : [];
    } catch {
      return [];
    }
  })(),

  togglePin: (id) =>
    set((s) => {
      const pins = s.pins.includes(id)
        ? s.pins.filter((value) => value !== id)
        : [...s.pins, id];
      localStorage.setItem("sidebar-pins", JSON.stringify(pins));
      return { pins };
    }),

  deleteInstance: async (id) => {
    const name = useStore.getState().instances.find((i) => i.id === id)?.name;
    await api.deleteInstance(id);
    notifyRemoved(name ? `Deleted ${name}` : "Instance deleted", "Its files are gone from disk");
    set(pruneInstances([id]));
  },

  deleteInstances: async (ids) => {
    const failures: string[] = [];
    const removed: string[] = [];
    for (const id of ids) {
      try {
        await api.deleteInstance(id);
        removed.push(id);
        set(pruneInstances([id]));
      } catch (error) {
        const name = useStore.getState().instances.find((i) => i.id === id)?.name ?? id;
        failures.push(`${name}: ${String(error)}`);
      }
    }
    if (removed.length > 0) {
      notifyRemoved(
        `Deleted ${removed.length} ${removed.length === 1 ? "instance" : "instances"}`,
        "Their files are gone from disk",
      );
    }
    return failures;
  },

  repairInstance: async (id) => api.repairInstance(id),

  duplicateInstance: async (id) => {
    const duplicate = await api.duplicateInstance(id);
    const installedVersions = await api.listInstalledVersions();
    set((state) => {
      const instances = [...state.instances, duplicate];
      return {
        instances,
        selectedInstanceId: duplicate.id,
        discoverTargetId: duplicate.id,
        installedIds: instances
          .filter((instance) => isInstanceInstalled(instance, installedVersions))
          .map((instance) => instance.id),
      };
    });
    await useStore.getState().refreshInstanceOrganization();
    return duplicate;
  },

  installInstance: async (id) => {
    try {
      await api.installInstance(id);
    } catch (e) {
      const message = String(e);
      if (/cancelled/i.test(message)) return;
      const name = useStore.getState().instances.find((i) => i.id === id)?.name;
      toast.error(name ? `Could not install ${name}` : "Install failed", {
        description: message,
      });
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
    if (processLogTimer) clearTimeout(processLogTimer);
    processLogTimer = null;
    pendingProcessLogs.clear();
    if (consoleTimer) clearTimeout(consoleTimer);
    consoleTimer = null;
    pendingConsole.clear();
    unlisteners.forEach((fn) => fn());
    unlisteners = [];
    listenersBound = false;
  });
}
