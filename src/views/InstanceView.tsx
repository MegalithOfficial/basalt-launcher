import { useCallback, useEffect, useState } from "react";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import {
  ArrowUpCircle,
  Check,
  FileBox,
  HardDriveUpload,
  Loader2,
  Package,
  Pencil,
  Plus,
  RefreshCw,
  Search,
  Share,
  Trash2,
} from "lucide-react";

import { EditInstanceModal } from "../components/EditInstanceModal";
import { ExportPackModal } from "../components/ExportPackModal";
import { InstallPlanPrompt } from "../components/InstallPlanPrompt";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { SuggestedContent } from "../components/SuggestedContent";
import { Select } from "../components/Select";
import { PlayButton } from "../components/PlayButton";
import { WorldsPanel } from "../components/worlds/WorldsPanel";
import { toast } from "sonner";

import { cn } from "../lib/cn";
import { api } from "../lib/api";
import { log } from "../lib/log";
import { notifyRemoved } from "../lib/notify";
import { loaderLabel } from "../lib/loader";
import { logoSrc, mediaSrc } from "../lib/media";
import { formatPlaytime, relativeTime } from "../lib/time";
import type {
  ContentItem,
  ContentKind,
  ContentUpdate,
  InstallPlan,
  ProjectSummary,
  RemovalPlan,
  SearchProvider,
} from "../lib/types";
import { useActiveProjectIds, useInstanceTask } from "../lib/useTasks";
import { useStore } from "../store";

type InstanceTab = ContentKind | "worlds";

const TABS: Array<{ kind: ContentKind; label: string; extensions: string[] }> = [
  { kind: "mods", label: "Mods", extensions: ["jar"] },
  { kind: "resourcepacks", label: "Resource Packs", extensions: ["zip"] },
  { kind: "shaderpacks", label: "Shaders", extensions: ["zip"] },
];

const SCHEMATICS_TAB = {
  kind: "schematics" as ContentKind,
  label: "Schematics",
  extensions: ["litematic", "schem", "schematic", "nbt"],
};

const WORLDS_TAB = {
  kind: "worlds" as const,
  label: "Worlds",
  extensions: [],
};

const SCHEMATIC_MOD_MARKERS = ["litematica", "worldedit", "schematica", "axiom", "schematic"];

const NO_UPDATES: ContentUpdate[] = [];
const EMPTY_ITEMS: ContentItem[] = [];
const ALL_KINDS = ["mods", "resourcepacks", "shaderpacks", "schematics"];

type ContentView = "all" | "enabled" | "disabled" | "updates" | "unlinked";
type ContentSort = "name" | "recent" | "size" | "updates" | "disabled";

const VIEWS: Array<{ id: ContentView; label: string }> = [
  { id: "all", label: "All" },
  { id: "enabled", label: "Enabled" },
  { id: "disabled", label: "Disabled" },
  { id: "updates", label: "Updates" },
  { id: "unlinked", label: "Unlinked" },
];

const SORT_LABELS: Record<ContentSort, string> = {
  name: "Name",
  recent: "Recently added",
  size: "Largest",
  updates: "Updates first",
  disabled: "Disabled first",
};

function displayName(item: ContentItem) {
  return (item.source?.title ?? item.file_name).toLowerCase();
}

function byName(a: ContentItem, b: ContentItem) {
  return displayName(a).localeCompare(displayName(b));
}

function sortItems(items: ContentItem[], sort: ContentSort) {
  const list = [...items];
  switch (sort) {
    case "recent":
      return list.sort(
        (a, b) => (b.source?.installed_at ?? 0) - (a.source?.installed_at ?? 0) || byName(a, b),
      );
    case "size":
      return list.sort((a, b) => b.size - a.size || byName(a, b));
    case "updates":
      return list.sort((a, b) => Number(!!b.update) - Number(!!a.update) || byName(a, b));
    case "disabled":
      return list.sort((a, b) => Number(a.enabled) - Number(b.enabled) || byName(a, b));
    default:
      return list.sort(byName);
  }
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function Toggle({
  on,
  onClick,
  disabled,
}: {
  on: boolean;
  onClick: () => void;
  disabled?: boolean;
}) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      aria-label={on ? "Disable" : "Enable"}
      className={cn(
        "relative h-5 w-9 shrink-0 rounded-full transition-colors duration-300",
        on ? "bg-[var(--accent)]" : "bg-surface-3",
        disabled && "cursor-not-allowed opacity-40",
      )}
    >
      <span
        className={cn(
          "absolute left-0.5 top-0.5 size-4 rounded-full bg-white shadow transition-transform duration-300",
          on ? "translate-x-4" : "translate-x-0",
        )}
      />
    </button>
  );
}

export function InstanceView() {
  const detailId = useStore((s) => s.detailInstanceId);
  const instance = useStore((s) => s.instances.find((i) => i.id === s.detailInstanceId));
  const media = useStore((s) => (detailId ? (s.media[detailId] ?? null) : null));
  const openSearch = useStore((s) => s.openSearch);
  const installContent = useStore((s) => s.installContent);
  const openProject = useStore((s) => s.openProject);
  const refreshContentSources = useStore((s) => s.refreshContentSources);
  const refreshUpdates = useStore((s) => s.refreshUpdates);
  const applyUpdate = useStore((s) => s.applyUpdate);
  const beginToastBatch = useStore((s) => s.beginToastBatch);
  const endToastBatch = useStore((s) => s.endToastBatch);
  const storedUpdates = useStore((s) => (detailId ? s.updates[detailId] : undefined));
  const gameRunning = useStore((s) =>
    Object.values(s.running).some(
      (running) => running.instance_id === detailId && running.state === "running",
    ),
  );
  const updates = storedUpdates ?? NO_UPDATES;
  const activeProjects = useActiveProjectIds();

  const [tab, setTab] = useState<InstanceTab>("mods");
  const [worldImport, setWorldImport] = useState(false);
  const [worldRefresh, setWorldRefresh] = useState(0);
  const [worldsLoading, setWorldsLoading] = useState(false);
  const [itemsByTab, setItemsByTab] = useState<Record<string, ContentItem[]>>({});
  const [loadingTab, setLoadingTab] = useState<string | null>(null);
  const [editOpen, setEditOpen] = useState(false);
  const [exportOpen, setExportOpen] = useState(false);
  const [launchError, setLaunchError] = useState<string | null>(null);
  const [hasSchematicMod, setHasSchematicMod] = useState(false);
  const [filter, setFilter] = useState("");
  const [listView, setListView] = useState<ContentView>("all");
  const [sort, setSort] = useState<ContentSort>(
    () => (localStorage.getItem("content-sort") as ContentSort) ?? "name",
  );
  const [checkingUpdates, setCheckingUpdates] = useState(false);
  const [updatingAll, setUpdatingAll] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState<{
    item: ContentItem;
    plan: RemovalPlan;
  } | null>(null);
  const [dropOrphans, setDropOrphans] = useState<string[]>([]);
  const [suggestPlan, setSuggestPlan] = useState<{
    provider: SearchProvider;
    project: ProjectSummary;
    plan: InstallPlan;
  } | null>(null);
  const [suggestBusy, setSuggestBusy] = useState<string | null>(null);

  const refresh = useCallback(
    async (reconcile = false) => {
      if (!instance || tab === "worlds") return;
      const target = tab;
      setLoadingTab(target);
      try {
        const listed = await api.listInstanceContent(instance.id, target, reconcile);
        setItemsByTab((current) => ({ ...current, [target]: listed }));
        void refreshContentSources(instance.id, target);
      } catch {
        setItemsByTab((current) => ({ ...current, [target]: [] }));
      } finally {
        setLoadingTab((current) => (current === target ? null : current));
      }
    },
    [instance?.id, tab, refreshContentSources],
  );

  useEffect(() => {
    if (!instance) return;
    const id = instance.id;
    let live = true;
    setItemsByTab({});
    setLoadingTab("*");
    api
      .listInstanceContentBundle(id, ALL_KINDS, true)
      .then((bundle) => {
        if (!live) return;
        setItemsByTab(bundle);
        for (const kind of ALL_KINDS) void refreshContentSources(id, kind);
      })
      .catch(() => {
        if (!live) return;
        setItemsByTab(Object.fromEntries(ALL_KINDS.map((kind) => [kind, []])));
      })
      .finally(() => {
        if (live) setLoadingTab(null);
      });
    return () => {
      live = false;
    };
  }, [instance?.id, refreshContentSources]);

  useEffect(() => {
    setTab(instance?.loader ? "mods" : "resourcepacks");
  }, [instance?.id]);

  useEffect(() => {
    if (instance) void refreshUpdates(instance.id);
  }, [instance?.id, refreshUpdates]);

  const modItems = itemsByTab.mods;

  useEffect(() => {
    if (!instance) return;
    if (modItems) {
      setHasSchematicMod(
        modItems.some((m) =>
          SCHEMATIC_MOD_MARKERS.some((marker) =>
            m.file_name.toLowerCase().includes(marker),
          ),
        ),
      );
      return;
    }
    let live = true;
    api
      .listInstanceContent(instance.id, "mods")
      .then((mods) => {
        if (!live) return;
        setHasSchematicMod(
          mods.some((m) =>
            SCHEMATIC_MOD_MARKERS.some((marker) =>
              m.file_name.toLowerCase().includes(marker),
            ),
          ),
        );
      })
      .catch(() => live && setHasSchematicMod(false));
    return () => {
      live = false;
    };
  }, [instance?.id, modItems]);

  if (!instance) {
    return (
      <div className="grid flex-1 place-items-center text-sm text-content-muted">
        Instance not found.
      </div>
    );
  }

  const baseTabs = instance.loader
    ? TABS
    : TABS.filter((t) => t.kind === "resourcepacks");
  const contentTabs = hasSchematicMod ? [...baseTabs, SCHEMATICS_TAB] : baseTabs;
  const allTabs = [...contentTabs, WORLDS_TAB];
  const tabMeta = allTabs.find((t) => t.kind === tab) ?? allTabs[0];
  const tabUpdates =
    tab === "worlds" ? NO_UPDATES : updates.filter((u) => u.kind === tab);
  const items = itemsByTab[tab] ?? EMPTY_ITEMS;
  const loading = loadingTab !== null && itemsByTab[tab] === undefined;
  const busyWithTask = !!useInstanceTask(instance?.id);
  const query = filter.trim().toLowerCase();
  const matching = query
    ? items.filter(
        (i) =>
          i.file_name.toLowerCase().includes(query) ||
          (i.source?.title ?? "").toLowerCase().includes(query),
      )
    : items;
  const shownItems = sortItems(
    matching.filter((i) => {
      if (listView === "enabled") return i.enabled;
      if (listView === "disabled") return !i.enabled;
      if (listView === "updates") return !!i.update;
      if (listView === "unlinked") return !i.source;
      return true;
    }),
    sort,
  );
  const enabledCount = items.filter((i) => i.enabled).length;
  const viewCounts: Record<ContentView, number> = {
    all: items.length,
    enabled: enabledCount,
    disabled: items.length - enabledCount,
    updates: items.filter((i) => !!i.update).length,
    unlinked: items.filter((i) => !i.source).length,
  };

  const addContent = async () => {
    if (tab === "worlds") return;
    if (tab !== "schematics") {
      openSearch(tab);
      return;
    }
    const files = await openFileDialog({
      multiple: true,
      directory: false,
      filters: [{ name: tabMeta.label, extensions: tabMeta.extensions }],
    });
    if (!files) return;
    const sources = Array.isArray(files) ? files : [files];
    await api.addInstanceContent(instance.id, tab, sources);
    await refresh();
  };

  const toggle = async (item: ContentItem) => {
    if (tab === "worlds") return;
    await api.toggleInstanceContent(instance.id, tab, item.file_name);
    await refresh();
  };

  const askRemove = async (item: ContentItem) => {
    if (tab === "worlds") return;
    const plan = await api
      .planContentRemoval(instance.id, tab, item.file_name)
      .catch(() => ({ dependents: [], from_pack: false, orphans: [] }) as RemovalPlan);
    if (plan.dependents.length === 0 && !plan.from_pack && plan.orphans.length === 0) {
      await remove(item, []);
      return;
    }
    setDropOrphans(plan.orphans.map((o) => o.file_name));
    setConfirmDelete({ item, plan });
  };

  const remove = async (item: ContentItem, alsoRemove: string[]) => {
    if (tab === "worlds") return;
    setConfirmDelete(null);
    await api.deleteInstanceContent(instance.id, tab, item.file_name);
    let extra = 0;
    for (const fileName of alsoRemove) {
      const dropped = await api
        .deleteInstanceContent(instance.id, tab, fileName)
        .then(() => true)
        .catch(() => false);
      if (dropped) extra += 1;
    }
    notifyRemoved(
      `Removed ${item.source?.title ?? item.file_name}`,
      extra > 0
        ? `and ${extra} unused ${extra === 1 ? "dependency" : "dependencies"} from ${instance.name}`
        : `from ${instance.name}`,
    );
    await refresh();
  };

  const installSuggestion = async (
    provider: SearchProvider,
    project: ProjectSummary,
    withDependencies = true,
    plan?: InstallPlan,
  ) => {
    if (tab === "worlds") return;
    setSuggestBusy(project.id);
    try {
      if (!plan) {
        const resolved = await api.planContentInstall(
          provider,
          project.id,
          instance.id,
          tab,
          instance.version_id,
          tab === "mods" ? instance.loader : null,
        );
        const replaces =
          !!resolved.primary?.replaces || resolved.dependencies.some((f) => !!f.replaces);
        const trivial =
          resolved.dependencies.length === 0 &&
          resolved.skipped.length === 0 &&
          resolved.conflicts.length === 0 &&
          !replaces;
        if (!trivial) {
          setSuggestPlan({ provider, project, plan: resolved });
          return;
        }
      }
      await installContent({
        provider,
        projectId: project.id,
        instanceId: instance.id,
        kind: tab,
        gameVersion: instance.version_id,
        loader: tab === "mods" ? instance.loader : null,
        withDependencies,
      });
      setSuggestPlan(null);
      setFilter("");
      await refresh();
    } catch (e) {
      setSuggestPlan(null);
      log.warn("content", `could not install ${project.title}: ${String(e)}`);
    } finally {
      setSuggestBusy(null);
    }
  };

  const checkUpdates = async () => {
    setCheckingUpdates(true);
    try {
      await refreshUpdates(instance.id, true);
      await refresh();
    } finally {
      setCheckingUpdates(false);
    }
  };

  const updateAll = async () => {
    setUpdatingAll(true);
    const total = tabUpdates.length;
    let done = 0;
    beginToastBatch();
    try {
      for (const update of tabUpdates) {
        try {
          await applyUpdate(instance.id, update.kind, update.file_name);
          done += 1;
        } catch (e) {
          toast.error(`Could not update ${update.latest_name}`, { description: String(e) });
        }
      }
      await refresh();
    } finally {
      endToastBatch(
        done > 0 ? `Updated ${done} of ${total} ${total === 1 ? "file" : "files"}` : null,
      );
      setUpdatingAll(false);
    }
  };

  const updateOne = async (item: ContentItem) => {
    if (tab === "worlds") return;
    try {
      await applyUpdate(instance.id, tab, item.file_name);
    } catch (e) {
      toast.error(`Could not update ${item.source?.title ?? item.file_name}`, {
        description: String(e),
      });
    }
    await refresh();
  };

  return (
    <div className="-mt-9 flex min-h-0 flex-1 flex-col">
      <div className="relative h-68 shrink-0 overflow-hidden">
        {media ? (
          <img
            src={mediaSrc(media)}
            className={cn(
              "absolute inset-0 h-full w-full object-cover",
              !media.local && "[image-rendering:pixelated]",
            )}
            draggable={false}
          />
        ) : (
          <div className="absolute inset-0 bg-surface-2" />
        )}
        <div className="absolute inset-0 bg-gradient-to-t from-base via-base/40 to-transparent" />
        <div className="absolute inset-0 bg-gradient-to-r from-black/60 via-transparent to-transparent" />

        <div className="absolute inset-x-0 bottom-0 flex items-end justify-between gap-4 px-6 pb-4">
          <div className="flex min-w-0 items-end gap-4">
            {logoSrc(instance.logo) && (
              <img
                src={logoSrc(instance.logo)!}
                alt=""
                className="size-16 shrink-0 rounded-2xl border border-white/10 bg-black/40 object-cover shadow-lg backdrop-blur"
                draggable={false}
              />
            )}
            <div className="min-w-0">
            <h1 className="truncate font-display text-3xl font-bold tracking-tight text-white drop-shadow">
              {instance.name}
            </h1>
            <div className="mt-1.5 flex items-center gap-2 text-[11px] text-white/60">
              <span className="rounded-md bg-black/50 px-2 py-0.5 font-pixel tracking-wider backdrop-blur">
                {instance.version_id} · {loaderLabel(instance).toUpperCase()}
                {instance.pack_project_id && " · MODPACK"}
              </span>
              {instance.last_played_at && (
                <span>
                  Played {relativeTime(instance.last_played_at)}
                  {formatPlaytime(instance.playtime_secs) &&
                    ` · ${formatPlaytime(instance.playtime_secs)}`}
                </span>
              )}
            </div>
            </div>
          </div>
          <div className="flex shrink-0 items-center gap-2">
            <PlayButton instance={instance} onError={setLaunchError} />
            <button
              onClick={() => setExportOpen(true)}
              aria-label="Export as pack"
              title="Export as pack"
              className="grid size-9 place-items-center rounded-full border border-white/10 bg-black/50 text-white/70 backdrop-blur transition-colors hover:bg-black/70 hover:text-white"
            >
              <Share className="size-4" />
            </button>
            <button
              onClick={() => setEditOpen(true)}
              aria-label="Edit instance"
              className="grid size-9 place-items-center rounded-full border border-white/10 bg-black/50 text-white/70 backdrop-blur transition-colors hover:bg-black/70 hover:text-white"
            >
              <Pencil className="size-4" />
            </button>
          </div>
        </div>
      </div>

      {launchError && (
        <div className="mx-6 mt-3 rounded-xl border border-danger/30 bg-danger/10 px-4 py-2.5 text-sm text-danger">
          {launchError}
        </div>
      )}

      <div className="flex items-center justify-between gap-4 border-b border-border-soft px-6 pt-4">
        <div className="flex gap-1">
          {allTabs.map((t) => {
            const count = updates.filter((u) => u.kind === t.kind).length;
            return (
              <button
                key={t.kind}
                onClick={() => setTab(t.kind)}
                className={cn(
                  "relative rounded-t-lg px-4 py-2.5 text-sm font-medium transition-colors",
                  tab === t.kind
                    ? "text-content"
                    : "text-content-faint hover:text-content-muted",
                )}
              >
                <span className="inline-flex items-center gap-1.5">
                  {t.label}
                  {count > 0 && (
                    <span className="rounded-full bg-warn/20 px-1.5 text-[10px] font-bold text-warn">
                      {count}
                    </span>
                  )}
                </span>
                {tab === t.kind && (
                  <span className="absolute inset-x-2 -bottom-px h-0.5 rounded-full bg-[var(--accent)] transition-colors duration-500" />
                )}
              </button>
            );
          })}
        </div>
        <div className="mb-2 flex items-center gap-2">
          {tab !== "worlds" && (
            <>
              {tabUpdates.length > 0 && (
                <button
                  onClick={updateAll}
                  disabled={updatingAll || busyWithTask}
                  title={busyWithTask ? "Wait for the current download to finish" : undefined}
                  className="inline-flex items-center gap-1.5 rounded-lg border border-warn/40 bg-warn/10 px-3 py-2 text-xs font-semibold text-warn transition-colors hover:bg-warn/20 disabled:cursor-not-allowed disabled:opacity-40"
                >
                  {updatingAll ? (
                    <Loader2 className="size-3.5 animate-spin" />
                  ) : (
                    <ArrowUpCircle className="size-3.5" />
                  )}
                  Update all ({tabUpdates.length})
                </button>
              )}
            </>
          )}
          <button
            onClick={() =>
              tab === "worlds" ? setWorldRefresh((v) => v + 1) : void checkUpdates()
            }
            disabled={
              tab === "worlds" ? worldsLoading : checkingUpdates || busyWithTask
            }
            title={tab === "worlds" ? "Refresh worlds" : "Check for updates"}
            aria-label={tab === "worlds" ? "Refresh worlds" : "Check for updates"}
            className="grid size-9 place-items-center rounded-lg border border-border bg-surface-2 text-content-faint transition-colors hover:bg-surface-3 hover:text-content disabled:cursor-not-allowed disabled:opacity-40"
          >
            <RefreshCw
              className={cn(
                "size-3.5",
                (tab === "worlds" ? worldsLoading : checkingUpdates) && "animate-spin",
              )}
            />
          </button>
          <button
            onClick={() => (tab === "worlds" ? setWorldImport(true) : void addContent())}
            disabled={busyWithTask}
            title={busyWithTask ? "Wait for the current download to finish" : undefined}
            className="inline-flex items-center gap-1.5 rounded-lg px-3 py-2 text-xs font-semibold text-black shadow-md shadow-[var(--accent-glow)] transition-all [background:linear-gradient(to_bottom,var(--accent),var(--accent-deep))] hover:[background:linear-gradient(to_bottom,var(--accent-bright),var(--accent))] disabled:cursor-not-allowed disabled:opacity-40 disabled:shadow-none"
          >
            {tab === "worlds" ? (
              <HardDriveUpload className="size-3.5" />
            ) : (
              <Plus className="size-3.5" />
            )}
            {tab === "worlds" ? "Import world" : "Add content"}
          </button>
        </div>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto">
        {tab === "worlds" ? (
          <WorldsPanel
            instance={instance}
            running={gameRunning}
            importOpen={worldImport}
            onImportOpenChange={setWorldImport}
            refreshToken={worldRefresh}
            onLoadingChange={setWorldsLoading}
          />
        ) : (
          <div className="px-6 py-5">
          {items.length > 0 && (
          <div className="mb-4 flex flex-wrap items-center gap-3">
            <div className="relative w-full max-w-xs">
              <Search className="absolute left-3 top-1/2 size-3.5 -translate-y-1/2 text-content-faint" />
              <input
                value={filter}
                onChange={(e) => setFilter(e.target.value)}
                placeholder={`Filter ${tabMeta.label.toLowerCase()}`}
                className="w-full rounded-lg border border-border bg-base py-2 pl-9 pr-3 text-sm text-content outline-none transition-colors focus:border-[var(--accent)]"
              />
            </div>

            <div
              role="group"
              aria-label="Show"
              className="flex shrink-0 items-center gap-0.5 rounded-lg border border-border-soft bg-surface-2/60 p-0.5"
            >
              {VIEWS.map((option) => {
                const count = viewCounts[option.id];
                if (option.id !== "all" && count === 0) return null;
                return (
                  <button
                    key={option.id}
                    onClick={() => setListView(option.id)}
                    className={cn(
                      "flex items-center gap-1.5 rounded-md px-2.5 py-1 text-xs font-medium transition-colors",
                      listView === option.id
                        ? "bg-surface-3 text-content"
                        : "text-content-faint hover:text-content-muted",
                    )}
                  >
                    {option.label}
                    {option.id !== "all" && (
                      <span
                        className={cn(
                          "tabular-nums text-[10px]",
                          option.id === "updates" ? "text-warn" : "text-content-faint",
                        )}
                      >
                        {count}
                      </span>
                    )}
                  </button>
                );
              })}
            </div>

            <div className="w-44 shrink-0">
              <Select
                compact
                label="Sort"
                value={SORT_LABELS[sort]}
                options={Object.values(SORT_LABELS)}
                onChange={(label) => {
                  const next =
                    (Object.keys(SORT_LABELS) as ContentSort[]).find(
                      (key) => SORT_LABELS[key] === label,
                    ) ?? "name";
                  setSort(next);
                  localStorage.setItem("content-sort", next);
                }}
              />
            </div>

            <span className="ml-auto shrink-0 text-xs tabular-nums text-content-faint">
              {shownItems.length === items.length
                ? `${items.length} ${items.length === 1 ? "file" : "files"}`
                : `${shownItems.length} of ${items.length}`}
              {enabledCount < items.length && ` · ${items.length - enabledCount} disabled`}
            </span>
          </div>
        )}

        {loading ? (
          <div className="flex items-center justify-center gap-2 py-12 text-sm text-content-muted">
            <Loader2 className="size-4 animate-spin" />
            Loading
          </div>
        ) : items.length === 0 ? (
          <div className="flex flex-col items-center gap-3 py-20 text-center">
            <div className="grid size-12 place-items-center rounded-2xl border border-border-soft bg-surface-2 text-content-faint">
              <Package className="size-6" />
            </div>
            <div className="text-sm font-medium text-content-muted">
              No {tabMeta.label.toLowerCase()} yet
            </div>
            <p className="max-w-sm text-xs text-content-faint">
              Browse Modrinth and CurseForge with Add content, or drop in your own files.
            </p>
            <button
              onClick={addContent}
              className="mt-1 inline-flex items-center gap-1.5 rounded-lg px-3 py-2 text-xs font-semibold text-black shadow-md shadow-[var(--accent-glow)] transition-all [background:linear-gradient(to_bottom,var(--accent),var(--accent-deep))] hover:[background:linear-gradient(to_bottom,var(--accent-bright),var(--accent))]"
            >
              <Plus className="size-3.5" />
              Add content
            </button>
          </div>
        ) : shownItems.length === 0 ? (
          <>
            <div
              className={cn(
                "text-sm text-content-faint",
                query ? "py-6" : "py-16 text-center",
              )}
            >
              {query
                ? `Nothing installed matches “${filter}”.`
                : `No ${listView === "unlinked" ? "unlinked" : listView} ${tabMeta.label.toLowerCase()}.`}
            </div>
            {query && (
              <SuggestedContent
                instance={instance}
                kind={tab}
                query={filter}
                busyId={suggestBusy}
                onInstall={(provider, project) => void installSuggestion(provider, project)}
                onOpen={(provider, project) =>
                  openProject(provider, project.id, tab, project.title)
                }
              />
            )}
          </>
        ) : (
          <div className="flex flex-col gap-1.5">
            {shownItems.map((item) => {
              const source = item.source;
              const displayName = source?.title ?? item.file_name;
              const linked = !!source?.provider && !!source.project_id;
              const busy = !!source?.project_id && activeProjects.has(source.project_id);
              return (
                <div
                  key={item.file_name}
                  className={cn(
                    "flex items-center gap-3 rounded-xl border px-4 py-2.5 transition-opacity",
                    item.update
                      ? "border-warn/30 bg-warn/[0.06]"
                      : "border-border-soft bg-surface-2/70",
                    !item.enabled && "opacity-55",
                  )}
                >
                  {source?.icon_url ? (
                    <img
                      src={source.icon_url}
                      loading="lazy"
                      className="size-9 shrink-0 rounded-lg bg-surface-3 object-cover"
                      draggable={false}
                    />
                  ) : (
                    <div className="grid size-9 shrink-0 place-items-center rounded-lg bg-surface-3 text-content-faint">
                      <FileBox className="size-4" />
                    </div>
                  )}
                  <div
                    className={cn("min-w-0 flex-1", linked && "cursor-pointer")}
                    onClick={() =>
                      linked &&
                      openProject(
                        source!.provider!,
                        source!.project_id!,
                        tab,
                        source!.title ?? undefined,
                      )
                    }
                  >
                    <div className="flex items-center gap-2">
                      <span className="truncate text-sm font-medium text-content">
                        {displayName}
                      </span>
                      {source?.provider && (
                        <span className="shrink-0 rounded bg-surface-3 px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-wide text-content-faint">
                          {source.provider}
                        </span>
                      )}
                      {source?.origin === "pack" && (
                        <span className="shrink-0 rounded bg-[var(--accent-glow)] px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-wide text-content-muted">
                          pack
                        </span>
                      )}
                      {source?.origin === "dependency" && (
                        <span className="shrink-0 rounded bg-surface-3 px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-wide text-content-faint">
                          dependency
                        </span>
                      )}
                      {!linked && source?.mod_id && (
                        <span
                          title="Identified from the file itself, not linked to a provider"
                          className="shrink-0 rounded bg-surface-3 px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-wide text-content-faint"
                        >
                          local
                        </span>
                      )}
                    </div>
                    <div className="truncate text-[11px] text-content-faint">
                      {source?.title ? `${item.file_name} · ` : ""}
                      {source?.mod_version && `v${source.mod_version} · `}
                      {formatSize(item.size)}
                      {!item.enabled && " · disabled"}
                    </div>
                  </div>

                  {item.update && (
                    <button
                      onClick={() => updateOne(item)}
                      disabled={busy || busyWithTask}
                      title={
                        busyWithTask
                          ? "Wait for the current download to finish"
                          : `Update to ${item.update.latest_name}`
                      }
                      className="inline-flex h-8 shrink-0 items-center gap-1.5 rounded-lg bg-warn/15 px-3 text-xs font-semibold text-warn transition-colors hover:bg-warn/25 disabled:cursor-not-allowed disabled:opacity-40"
                    >
                      {busy ? (
                        <Loader2 className="size-3.5 animate-spin" />
                      ) : (
                        <ArrowUpCircle className="size-3.5" />
                      )}
                      Update
                    </button>
                  )}

                  <Toggle
                    on={item.enabled}
                    disabled={busyWithTask}
                    onClick={() => toggle(item)}
                  />
                  <button
                    onClick={() => askRemove(item)}
                    disabled={busyWithTask}
                    aria-label="Delete file"
                    title={busyWithTask ? "Wait for the current download to finish" : undefined}
                    className="grid size-8 place-items-center rounded-lg text-content-faint transition-colors hover:bg-danger/15 hover:text-danger disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:bg-transparent disabled:hover:text-content-faint"
                  >
                    <Trash2 className="size-4" />
                  </button>
                </div>
              );
            })}
          </div>
          )}
          </div>
        )}
      </div>

      <ConfirmDialog
        open={!!confirmDelete}
        nested
        tone={confirmDelete && confirmDelete.plan.dependents.length > 0 ? "danger" : "warn"}
        title={
          confirmDelete
            ? `Remove ${confirmDelete.item.source?.title ?? confirmDelete.item.file_name}?`
            : ""
        }
        description={
          confirmDelete ? (
            confirmDelete.plan.dependents.length > 0 ? (
              <>
                <span className="font-medium text-danger">
                  {confirmDelete.plan.dependents.join(", ")}
                </span>{" "}
                {confirmDelete.plan.dependents.length === 1 ? "requires" : "require"} this file.
                Removing it will likely break the game.
              </>
            ) : confirmDelete.plan.from_pack ? (
              "This file came from a modpack. Removing it may break the pack."
            ) : (
              "This file brought other mods in with it."
            )
          ) : null
        }
        cancelLabel="Keep it"
        confirmLabel={
          dropOrphans.length > 0
            ? `Remove ${dropOrphans.length + 1} files`
            : confirmDelete && confirmDelete.plan.dependents.length > 0
              ? "Remove anyway"
              : "Remove"
        }
        onConfirm={async () => {
          if (confirmDelete) await remove(confirmDelete.item, dropOrphans);
        }}
        onCancel={() => setConfirmDelete(null)}
      >
        {confirmDelete && confirmDelete.plan.orphans.length > 0 ? (
          <>
            <div className="text-xs font-medium text-content">
              {confirmDelete.plan.orphans.length === 1
                ? "It installed one dependency that nothing else needs"
                : `It installed ${confirmDelete.plan.orphans.length} dependencies that nothing else needs`}
            </div>
            <div className="mt-2.5 flex flex-col gap-1">
              {confirmDelete.plan.orphans.map((orphan) => {
                const checked = dropOrphans.includes(orphan.file_name);
                return (
                  <button
                    key={orphan.file_name}
                    onClick={() =>
                      setDropOrphans((current) =>
                        checked
                          ? current.filter((f) => f !== orphan.file_name)
                          : [...current, orphan.file_name],
                      )
                    }
                    className="flex items-center gap-2.5 rounded-lg px-1.5 py-1.5 text-left transition-colors hover:bg-surface-2"
                  >
                    <span
                      className={cn(
                        "grid size-4 shrink-0 place-items-center rounded border",
                        checked
                          ? "border-danger bg-danger/20 text-danger"
                          : "border-border bg-surface-3",
                      )}
                    >
                      {checked && <Check className="size-3" strokeWidth={3} />}
                    </span>
                    {orphan.icon_url ? (
                      <img
                        src={orphan.icon_url}
                        alt=""
                        className="size-6 shrink-0 rounded bg-surface-3 object-cover"
                        draggable={false}
                      />
                    ) : (
                      <span className="grid size-6 shrink-0 place-items-center rounded bg-surface-3 text-content-faint">
                        <Package className="size-3" />
                      </span>
                    )}
                    <span className="min-w-0 flex-1 truncate text-xs text-content-muted">
                      {orphan.title}
                    </span>
                  </button>
                );
              })}
            </div>
          </>
        ) : null}
      </ConfirmDialog>

      <InstallPlanPrompt
        plan={suggestPlan?.plan ?? null}
        busy={suggestBusy !== null && suggestPlan !== null}
        progress={null}
        onConfirm={() =>
          suggestPlan &&
          void installSuggestion(
            suggestPlan.provider,
            suggestPlan.project,
            true,
            suggestPlan.plan,
          )
        }
        onSkipDependencies={() =>
          suggestPlan &&
          void installSuggestion(
            suggestPlan.provider,
            suggestPlan.project,
            false,
            suggestPlan.plan,
          )
        }
        onCancel={() => setSuggestPlan(null)}
      />

      <ExportPackModal
        instance={exportOpen ? instance : null}
        onClose={() => setExportOpen(false)}
      />
      <EditInstanceModal
        instance={editOpen ? instance : null}
        onClose={() => setEditOpen(false)}
      />
    </div>
  );
}
