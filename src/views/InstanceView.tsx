import { useCallback, useEffect, useRef, useState } from "react";
import { openPath } from "@tauri-apps/plugin-opener";
import {
  ArrowUpCircle,
  Check,
  ClipboardCopy,
  DatabaseBackup,
  FileBox,
  HardDriveUpload,
  ChevronDown,
  Compass,
  FileUp,
  Copy,
  FolderOpen,
  Loader2,
  MoreVertical,
  Pin,
  PinOff,
  Settings,
  Package,
  Plus,
  RefreshCw,
  Search,
  SearchX,
  Share,
  Trash2,
  Wrench,
} from "lucide-react";

import { Banner } from "../components/Banner";
import { EditInstanceModal } from "../components/EditInstanceModal";
import { ExportPackModal } from "../components/ExportPackModal";
import { InstallPlanPrompt } from "../components/InstallPlanPrompt";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { SuggestedContent } from "../components/SuggestedContent";
import { DeferredImage } from "../components/DeferredImage";
import { useCurseforgeDownloads } from "../components/CurseForgeDownloadModal";
import { Select } from "../components/Select";
import { PlayButton } from "../components/PlayButton";
import { WorldsPanel } from "../components/worlds/WorldsPanel";
import { ScreenshotsPanel } from "../components/captures/ScreenshotsPanel";
import { DatapacksPanel } from "../components/datapacks/DatapacksPanel";
import { UploadModal } from "../components/UploadModal";
import { ContextMenu, useContextMenu, type MenuItem } from "../components/ContextMenu";
import { SnapshotsModal } from "../components/SnapshotsModal";
import { ModpackUpgradeModal } from "../components/ModpackUpgradeModal";
import { toast } from "sonner";

import { cn } from "../lib/cn";
import { api } from "../lib/api";
import { log } from "../lib/log";
import { notifyRemoved } from "../lib/notify";
import { loaderLabel } from "../lib/loader";
import { logoSrc } from "../lib/media";
import { formatPlaytime, relativeTime } from "../lib/time";
import type {
  ContentItem,
  ContentKind,
  ContentUpdate,
  InstallPlan,
  ManualDownloadSource,
  ModpackUpgrade,
  ModpackUpgradePlan,
  ProjectSummary,
  RemovalPlan,
  SearchProvider,
} from "../lib/types";
import { useActiveProjectIds, useInstanceTask } from "../lib/useTasks";
import { useStore } from "../store";
import { formatBytes } from "../lib/format";

type InstanceTab = ContentKind | "worlds" | "screenshots";

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

const DATAPACKS_TAB = {
  kind: "datapacks" as ContentKind,
  label: "Datapacks",
  extensions: ["zip"],
};

const SCREENSHOTS_TAB = {
  kind: "screenshots" as const,
  label: "Screenshots",
  extensions: [],
};

const SCHEMATIC_MOD_MARKERS = ["litematica", "worldedit", "schematica", "axiom", "schematic"];

function isContentTab(tab: InstanceTab): tab is ContentKind {
  return tab !== "worlds" && tab !== "screenshots" && tab !== "datapacks";
}

const NO_UPDATES: ContentUpdate[] = [];
const EMPTY_ITEMS: ContentItem[] = [];
const ALL_KINDS = ["mods", "resourcepacks", "shaderpacks", "schematics"];

type Dialog =
  | { kind: "edit" }
  | { kind: "delete" }
  | { kind: "export" }
  | { kind: "snapshots" }
  | { kind: "packUpgrade"; plan: ModpackUpgradePlan; sources: ManualDownloadSource[] }
  | { kind: "worldImport" }
  | { kind: "remove"; item: ContentItem; plan: RemovalPlan; orphans: string[] }
  | {
      kind: "install";
      provider: SearchProvider;
      project: ProjectSummary;
      plan: InstallPlan;
    };

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
        on ? "bg-(--accent)" : "bg-surface-3",
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
  const repairInstance = useStore((s) => s.repairInstance);
  const refreshInstances = useStore((s) => s.refreshInstances);
  const applyUpdate = useStore((s) => s.applyUpdate);
  const beginOptimisticTask = useStore((s) => s.beginOptimisticTask);
  const endOptimisticTask = useStore((s) => s.endOptimisticTask);
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
  const suggestionsEnabled = useStore((s) => s.settings?.show_suggestions !== false);
  const duplicateInstance = useStore((s) => s.duplicateInstance);
  const openDiscover = useStore((s) => s.openDiscover);
  const deleteInstance = useStore((s) => s.deleteInstance);
  const togglePin = useStore((s) => s.togglePin);
  const pinned = useStore((s) => s.pins.includes(s.detailInstanceId ?? ""));
  const busyWithTask = !!useInstanceTask(instance?.id);
  const { menu, open: openMenu, close: closeMenu } = useContextMenu();

  const [tab, setTab] = useState<InstanceTab>("mods");
  const [dialog, setDialog] = useState<Dialog | null>(null);
  const [worldRefresh, setWorldRefresh] = useState(0);
  const [worldsLoading, setWorldsLoading] = useState(false);
  const [addingFiles, setAddingFiles] = useState(false);
  const [addingBusy, setAddingBusy] = useState(false);
  const [datapackAdd, setDatapackAdd] = useState(0);
  const [addingError, setAddingError] = useState<string | null>(null);
  const [itemsByTab, setItemsByTab] = useState<Record<string, ContentItem[]>>({});
  const [loadingTab, setLoadingTab] = useState<string | null>(null);
  const [launchError, setLaunchError] = useState<string | null>(null);
  const [hasSchematicMod, setHasSchematicMod] = useState(false);
  const [filter, setFilter] = useState("");
  const [listView, setListView] = useState<ContentView>("all");
  const [sort, setSort] = useState<ContentSort>(
    () => (localStorage.getItem("content-sort") as ContentSort) ?? "name",
  );
  const [checkingUpdates, setCheckingUpdates] = useState(false);
  const [updatingAll, setUpdatingAll] = useState(false);
  const [updatingFile, setUpdatingFile] = useState<string | null>(null);
  const [suggestBusy, setSuggestBusy] = useState<string | null>(null);
  const [repairing, setRepairing] = useState(false);
  const [checkingPackUpgrade, setCheckingPackUpgrade] = useState(false);
  const [packUpgrade, setPackUpgrade] = useState<ModpackUpgrade | null>(null);
  const [upgradingPack, setUpgradingPack] = useState(false);
  const installingInstance = useRef<string | null>(null);
  const browserDownloads = useCurseforgeDownloads();

  const close = () => setDialog(null);
  const removal = dialog?.kind === "remove" ? dialog : null;
  const suggestion = dialog?.kind === "install" ? dialog : null;

  const refresh = useCallback(
    async (reconcile = false) => {
      if (!instance || !isContentTab(tab)) return;
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
      .listInstanceContentBundle(id, ALL_KINDS, false)
      .then((bundle) => {
        if (!live) return;
        setItemsByTab(bundle);
        for (const kind of ALL_KINDS) void refreshContentSources(id, kind);
        void (async () => {
          for (const kind of ALL_KINDS) {
            try {
              const reconciled = await api.listInstanceContent(id, kind, true);
              if (!live) return;
              setItemsByTab((current) => ({ ...current, [kind]: reconciled }));
            } catch {}
          }
        })();
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

  useEffect(() => {
    if (!instance) return;
    if (busyWithTask) {
      installingInstance.current = instance.id;
      return;
    }
    if (installingInstance.current !== instance.id) return;
    installingInstance.current = null;

    let live = true;
    api
      .listInstanceContentBundle(instance.id, ALL_KINDS, false)
      .then((bundle) => {
        if (!live) return;
        setItemsByTab(bundle);
        for (const kind of ALL_KINDS) void refreshContentSources(instance.id, kind);
        void refreshUpdates(instance.id);
      })
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [busyWithTask, instance?.id, refreshContentSources, refreshUpdates]);

  useEffect(() => {
    if (!instance || !busyWithTask || !isContentTab(tab)) return;
    const instanceId = instance.id;
    const kind = tab;
    let live = true;
    let reading = false;

    const refreshVisibleContent = async () => {
      if (reading) return;
      reading = true;
      try {
        const listed = await api.listInstanceContent(instanceId, kind);
        if (live) setItemsByTab((current) => ({ ...current, [kind]: listed }));
      } catch {}
      reading = false;
    };

    void refreshVisibleContent();
    const timer = window.setInterval(() => void refreshVisibleContent(), 1_000);
    return () => {
      live = false;
      window.clearInterval(timer);
    };
  }, [busyWithTask, instance?.id, tab]);

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
  const allTabs = [...contentTabs, DATAPACKS_TAB, WORLDS_TAB, SCREENSHOTS_TAB];
  const isContent = tab !== "worlds" && tab !== "screenshots" && tab !== "datapacks";
  const tabMeta = allTabs.find((t) => t.kind === tab) ?? allTabs[0];
  const tabUpdates = isContent ? updates.filter((u) => u.kind === tab) : NO_UPDATES;
  const items = itemsByTab[tab] ?? EMPTY_ITEMS;
  const loading = loadingTab !== null && itemsByTab[tab] === undefined;
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

  const addContent = (event: React.MouseEvent) => {
    if (!isContentTab(tab)) return;
    if (tab === "schematics") {
      setAddingFiles(true);
      return;
    }
    openMenu(
      event,
      [
        {
          label: `Browse ${tabMeta.label.toLowerCase()}`,
          icon: Compass,
          onSelect: () => openSearch(tab),
        },
        {
          label: "Add files from disk",
          icon: FileUp,
          onSelect: () => setAddingFiles(true),
        },
      ],
      undefined,
      { below: true },
    );
  };

  const addFiles = async (sources: string[]) => {
    if (!isContentTab(tab)) return;
    setAddingBusy(true);
    setAddingError(null);
    try {
      await api.addInstanceContent(instance.id, tab, sources);
      setAddingFiles(false);
      await refresh();
    } catch (cause) {
      setAddingError(String(cause));
    } finally {
      setAddingBusy(false);
    }
  };

  const toggle = async (item: ContentItem) => {
    if (!isContentTab(tab)) return;
    await api.toggleInstanceContent(instance.id, tab, item.file_name);
    await refresh();
  };

  const askRemove = async (item: ContentItem) => {
    if (!isContentTab(tab)) return;
    const plan = await api
      .planContentRemoval(instance.id, tab, item.file_name)
      .catch(() => ({ dependents: [], from_pack: false, orphans: [] }) as RemovalPlan);
    if (plan.dependents.length === 0 && !plan.from_pack && plan.orphans.length === 0) {
      await remove(item, []);
      return;
    }
    setDialog({
      kind: "remove",
      item,
      plan,
      orphans: plan.orphans.map((o) => o.file_name),
    });
  };

  const remove = async (item: ContentItem, alsoRemove: string[]) => {
    if (!isContentTab(tab)) return;
    close();
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
    if (!isContentTab(tab)) return;
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
          setDialog({ kind: "install", provider, project, plan: resolved });
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
      close();
      setFilter("");
      await refresh();
    } catch (e) {
      close();
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

  const applyAvailableUpdate = async (update: ContentUpdate) => {
    const requirement = await api.planContentUpdate(
      instance.id,
      update.kind,
      update.file_name,
    );
    let sources = undefined;
    let optimisticTask: string | null = null;
    if (requirement) {
      const downloaded = await browserDownloads.collect([requirement]);
      if (!downloaded) return false;
      sources = downloaded;
      const item = items.find((item) => item.file_name === update.file_name);
      optimisticTask = beginOptimisticTask(
        "content_update",
        item?.source?.title ?? update.latest_name,
        {
          subtitle: `Updating ${instance.name}`,
          iconUrl: item?.source?.icon_url ?? null,
          instanceId: instance.id,
          projectId: item?.source?.project_id ?? null,
        },
      );
      toast.info("Browser download verified", {
        description: "Installing the update.",
      });
    }
    try {
      await applyUpdate(instance.id, update.kind, update.file_name, sources);
      return true;
    } finally {
      if (optimisticTask) endOptimisticTask(optimisticTask);
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
          if (!(await applyAvailableUpdate(update))) break;
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
    if (!isContentTab(tab) || !item.update) return;
    setUpdatingFile(item.file_name);
    try {
      await applyAvailableUpdate(item.update);
    } catch (e) {
      toast.error(`Could not update ${item.source?.title ?? item.file_name}`, {
        description: String(e),
      });
    } finally {
      setUpdatingFile(null);
      await refresh();
    }
  };

  useEffect(() => {
    if (!instance.pack_project_id) {
      setPackUpgrade(null);
      return;
    }
    let live = true;
    api
      .checkModpackUpgrade(instance.id)
      .then((update) => {
        if (!live) return;
        setPackUpgrade(update);
        if (!update) return;
        const key = `pack-update-seen:${instance.id}`;
        if (localStorage.getItem(key) === update.target_version_id) return;
        localStorage.setItem(key, update.target_version_id);
        toast.info(`Modpack update ${update.version_number} is out`, {
          description: "Open it from the instance to see what changed before applying.",
        });
      })
      .catch(() => live && setPackUpgrade(null));
    return () => {
      live = false;
    };
  }, [instance.id, instance.pack_project_id, instance.pack_version_id]);

  const heroMenu = (): MenuItem[] => [
    ...(instance.pack_project_id
      ? [
          {
            label: packUpgrade
              ? `Upgrade to ${packUpgrade.version_number}`
              : checkingPackUpgrade
                ? "Checking for pack update"
                : "Check for pack update",
            icon: checkingPackUpgrade ? Loader2 : ArrowUpCircle,
            disabled: checkingPackUpgrade || busyWithTask || gameRunning,
            onSelect: () =>
              packUpgrade
                ? void openPackUpgrade(packUpgrade.target_version_id)
                : void checkPackUpgrade(),
          } satisfies MenuItem,
        ]
      : []),
    {
      label: instance.loader ? "Find mods" : "Find mods (requires loader)",
      icon: Compass,
      disabled: !instance.loader,
      onSelect: () => openDiscover("mods", instance.id),
    },
    {
      label: "Open folder",
      icon: FolderOpen,
      onSelect: () => void openPath(instance.dir),
    },
    {
      label: "Snapshots and restore (experimental)",
      icon: DatabaseBackup,
      onSelect: () => setDialog({ kind: "snapshots" }),
    },
    {
      label: "Repair and verify",
      icon: Wrench,
      disabled: repairing || busyWithTask || gameRunning,
      onSelect: () => void repair(),
    },
    {
      label: "Export as pack",
      icon: Share,
      onSelect: () => setDialog({ kind: "export" }),
    },
    {
      label: "Duplicate instance",
      icon: Copy,
      disabled: gameRunning || busyWithTask,
      onSelect: () => {
        duplicateInstance(instance.id).catch((error) =>
          toast.error(`Could not duplicate ${instance.name}`, { description: String(error) }),
        );
      },
    },
    {
      label: "Copy launch command",
      icon: ClipboardCopy,
      onSelect: () => {
        api
          .getInstanceLaunchCommand(instance.id)
          .then(async (option) => {
            await navigator.clipboard.writeText(option);
            toast.success("Copied launch command", { description: option });
          })
          .catch((error) =>
            toast.error("Could not copy the launch command", { description: String(error) }),
          );
      },
    },
    {
      label: "Settings",
      icon: Settings,
      separated: true,
      onSelect: () => setDialog({ kind: "edit" }),
    },
    {
      label: pinned ? "Unpin from sidebar" : "Pin to sidebar",
      icon: pinned ? PinOff : Pin,
      onSelect: () => togglePin(instance.id),
    },
    {
      label: "Delete instance",
      icon: Trash2,
      danger: true,
      separated: true,
      onSelect: () => setDialog({ kind: "delete" }),
    },
  ];

  const openPackUpgrade = async (targetVersionId: string) => {
    setCheckingPackUpgrade(true);
    try {
      let sources: ManualDownloadSource[] = [];
      let plan = await api.planModpackUpgrade(instance.id, targetVersionId);
      while (plan.manual_downloads.length > 0) {
        const downloaded = await browserDownloads.collect(plan.manual_downloads);
        if (!downloaded) return;
        sources = [...sources, ...downloaded];
        plan = await api.planModpackUpgrade(instance.id, targetVersionId, sources);
      }
      setDialog({ kind: "packUpgrade", plan, sources });
    } catch (error) {
      toast.error("Could not read the update", { description: String(error) });
    } finally {
      setCheckingPackUpgrade(false);
    }
  };

  const checkPackUpgrade = async () => {
    setCheckingPackUpgrade(true);
    try {
      const update = await api.checkModpackUpgrade(instance.id);
      setPackUpgrade(update);
      if (!update) {
        toast.success("Modpack is up to date", {
          description: "The latest available pack version is already installed.",
        });
        return;
      }
      await openPackUpgrade(update.target_version_id);
    } catch (error) {
      toast.error("Could not check the modpack", { description: String(error) });
    } finally {
      setCheckingPackUpgrade(false);
    }
  };

  const confirmPackUpgrade = async (snapshotFirst: boolean) => {
    if (dialog?.kind !== "packUpgrade") return;
    const { plan, sources } = dialog;
    setUpgradingPack(true);
    setDialog(null);
    toast.info(`Upgrading to ${plan.update.version_number}`, {
      description: snapshotFirst
        ? "Taking a snapshot first, then applying the pack."
        : "Applying the pack without a snapshot.",
    });
    const optimistic = beginOptimisticTask("modpack_upgrade", `Upgrade ${instance.name}`, {
      subtitle: `Preparing ${plan.update.version_number}`,
      instanceId: instance.id,
      projectId: instance.pack_project_id,
    });
    try {
      await api.upgradeModpack(
        instance.id,
        plan.update.target_version_id,
        sources,
        snapshotFirst,
      );
      toast.success(`Upgraded ${instance.name}`, {
        description: `${plan.update.version_number} is ready. Your previous state is available in Snapshots.`,
      });
      setPackUpgrade(null);
      await refreshInstances();
      await refresh();
    } catch (error) {
      if (!/cancelled/i.test(String(error))) {
        toast.error(`Could not upgrade ${instance.name}`, {
          description: `${String(error)} The existing instance was kept unchanged.`,
        });
      }
    } finally {
      endOptimisticTask(optimistic);
      setUpgradingPack(false);
    }
  };

  const repair = async () => {
    setRepairing(true);
    try {
      const report = await repairInstance(instance.id);
      if (report.unresolved.length > 0) {
        toast.warning(
          `Repair finished with ${report.unresolved.length} unresolved ${report.unresolved.length === 1 ? "file" : "files"}`,
          { description: report.unresolved.slice(0, 3).join(" · ") },
        );
      } else {
        toast.success("Instance verified", {
          description:
            report.repaired_content > 0
              ? `Repaired ${report.repaired_content} tracked ${report.repaired_content === 1 ? "file" : "files"}.`
              : `Game files and ${report.checked_content} tracked content files are healthy.`,
        });
      }
      await refresh();
    } catch (error) {
      if (!/cancelled/i.test(String(error))) {
        toast.error(`Could not repair ${instance.name}`, { description: String(error) });
      }
    } finally {
      setRepairing(false);
    }
  };

  return (
    <div className="-mt-9 flex min-h-0 flex-1 flex-col">
      <div className="relative h-68 shrink-0 overflow-hidden">
        {media ? (
          <Banner media={media} className="absolute inset-0 h-full w-full" />
        ) : (
          <div className="absolute inset-0 bg-surface-2" />
        )}
        <div className="absolute inset-0 bg-linear-to-t from-void via-void/40 to-transparent" />
        <div className="absolute inset-0 bg-linear-to-r from-black/60 via-transparent to-transparent" />

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
            <button
              onClick={(event) => openMenu(event, heroMenu(), instance.name)}
              aria-label="Instance actions"
              title="Instance actions"
              className="grid size-10 place-items-center rounded-full border border-white/10 bg-black/50 text-white/70 backdrop-blur transition-colors hover:bg-black/70 hover:text-white"
            >
              {repairing ? (
                <Loader2 className="size-4 animate-spin" />
              ) : (
                <MoreVertical className="size-4" />
              )}
            </button>
            {packUpgrade && (
              <button
                onClick={() => void openPackUpgrade(packUpgrade.target_version_id)}
                disabled={checkingPackUpgrade || busyWithTask || gameRunning}
                title={`${packUpgrade.target_name} ${packUpgrade.version_number} was released`}
                className="inline-flex h-10 items-center gap-2 rounded-full px-4 text-xs font-semibold text-black shadow-lg shadow-(color:--accent-glow) transition-all hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-50 [background:linear-gradient(to_bottom,var(--accent),var(--accent-deep))]"
              >
                {checkingPackUpgrade ? (
                  <Loader2 className="size-4 animate-spin" />
                ) : (
                  <ArrowUpCircle className="size-4" />
                )}
                Update to {packUpgrade.version_number}
              </button>
            )}
            <div className="ml-1">
              <PlayButton instance={instance} hero onError={setLaunchError} />
            </div>
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
                  <span className="absolute inset-x-2 -bottom-px h-0.5 rounded-full bg-(--accent) transition-colors duration-500" />
                )}
              </button>
            );
          })}
        </div>
        <div className="mb-2 flex items-center gap-2">
          {isContent && (
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
          {tab !== "screenshots" && (
            <>
              <button
                onClick={() =>
                  isContentTab(tab) ? void checkUpdates() : setWorldRefresh((v) => v + 1)
                }
                disabled={isContentTab(tab) ? checkingUpdates || busyWithTask : worldsLoading}
                title={isContentTab(tab) ? "Check for updates" : "Refresh"}
                aria-label={isContentTab(tab) ? "Check for updates" : "Refresh"}
                className="grid size-9 place-items-center rounded-lg border border-border bg-surface-2 text-content-faint transition-colors hover:bg-surface-3 hover:text-content disabled:cursor-not-allowed disabled:opacity-40"
              >
                <RefreshCw
                  className={cn(
                    "size-3.5",
                    (isContentTab(tab) ? checkingUpdates : worldsLoading) && "animate-spin",
                  )}
                />
              </button>
              <button
                onClick={(event) =>
                  tab === "worlds" ? setDialog({ kind: "worldImport" }) : addContent(event)
                }
                disabled={busyWithTask}
                title={busyWithTask ? "Wait for the current download to finish" : undefined}
                className="inline-flex items-center gap-1.5 rounded-lg px-3 py-2 text-xs font-semibold text-black shadow-md shadow-(color:--accent-glow) transition-all [background:linear-gradient(to_bottom,var(--accent),var(--accent-deep))] hover:[background:linear-gradient(to_bottom,var(--accent-bright),var(--accent))] disabled:cursor-not-allowed disabled:opacity-40 disabled:shadow-none"
              >
                {tab === "worlds" ? (
                  <HardDriveUpload className="size-3.5" />
                ) : (
                  <Plus className="size-3.5" />
                )}
                {tab === "worlds" ? "Import world" : "Add content"}
                {tab !== "worlds" && tab !== "schematics" && (
                  <ChevronDown className="size-3.5 opacity-70" />
                )}
              </button>
            </>
          )}
        </div>
      </div>

      <div className="flex min-h-0 flex-1 flex-col overflow-y-auto">
        {tab === "datapacks" ? (
          <DatapacksPanel
            instance={instance}
            refreshToken={worldRefresh}
            addFor={datapackAdd}
            onAddHandled={() => setDatapackAdd(0)}
          />
        ) : tab === "screenshots" ? (
          <ScreenshotsPanel instance={instance} />
        ) : tab === "worlds" ? (
          <WorldsPanel
            instance={instance}
            running={gameRunning}
            importOpen={dialog?.kind === "worldImport"}
            onImportOpenChange={(open) => setDialog(open ? { kind: "worldImport" } : null)}
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
                className="w-full rounded-lg border border-border bg-void py-2 pl-9 pr-3 text-sm text-content outline-none transition-colors focus:border-(--accent)"
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
              {busyWithTask
                ? "Installed files will appear here as the current task progresses."
                : "Browse Modrinth and CurseForge with Add content, or drop in your own files."}
            </p>
            <button
              onClick={addContent}
              disabled={busyWithTask}
              className="mt-1 inline-flex items-center gap-1.5 rounded-lg px-3 py-2 text-xs font-semibold text-black shadow-md shadow-(color:--accent-glow) transition-all [background:linear-gradient(to_bottom,var(--accent),var(--accent-deep))] hover:[background:linear-gradient(to_bottom,var(--accent-bright),var(--accent))] disabled:cursor-not-allowed disabled:opacity-40 disabled:shadow-none"
            >
              <Plus className="size-3.5" />
              Add content
            </button>
          </div>
        ) : shownItems.length === 0 ? (
          <>
            {query ? (
              <div className="flex items-center gap-3.5 pb-7 pt-9">
                <div className="grid size-11 shrink-0 place-items-center rounded-xl border border-border-soft bg-surface-2 text-content-faint">
                  <SearchX className="size-5" />
                </div>
                <div className="min-w-0 flex-1">
                  <div className="truncate text-sm font-medium text-content">
                    Nothing installed matches “{filter}”
                  </div>
                  <div className="mt-0.5 text-xs text-content-faint">
                    {items.length === 0
                      ? `This instance has no ${tabMeta.label.toLowerCase()} yet.`
                      : `Searched ${items.length} installed ${
                          items.length === 1 ? "file" : "files"
                        }${listView === "all" ? "" : ` in ${listView}`}.`}
                  </div>
                </div>
                <button
                  onClick={() => setFilter("")}
                  className="shrink-0 rounded-lg border border-border bg-surface-2 px-3 py-1.5 text-xs font-medium text-content-muted transition-colors hover:bg-surface-3 hover:text-content"
                >
                  Clear search
                </button>
              </div>
            ) : (
              <div className="py-16 text-center text-sm text-content-faint">
                {`No ${listView === "unlinked" ? "unlinked" : listView} ${tabMeta.label.toLowerCase()}.`}
              </div>
            )}
            {query && suggestionsEnabled && (
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
              const busy =
                (!!source?.project_id && activeProjects.has(source.project_id)) ||
                updatingAll ||
                updatingFile !== null;
              return (
                <div
                  key={item.file_name}
                  className={cn(
                    "flex items-center gap-3 rounded-xl border px-4 py-2.5 transition-opacity",
                    item.update
                      ? "border-warn/30 bg-warn/6"
                      : "border-border-soft bg-surface-2/70",
                    !item.enabled && "opacity-55",
                  )}
                >
                  {source?.icon_url ? (
                    <DeferredImage
                      src={source.icon_url}
                      alt=""
                      className="size-9 shrink-0 rounded-lg bg-surface-3 object-cover"
                      fallback={
                        <div className="grid size-9 shrink-0 place-items-center rounded-lg bg-surface-3 text-content-faint">
                          <FileBox className="size-4" />
                        </div>
                      }
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
                        <span className="shrink-0 rounded bg-(--accent-glow) px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-wide text-content-muted">
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
                      {formatBytes(item.size)}
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
        open={!!removal}
        nested
        tone={removal && removal.plan.dependents.length > 0 ? "danger" : "warn"}
        title={
          removal
            ? `Remove ${removal.item.source?.title ?? removal.item.file_name}?`
            : ""
        }
        description={
          removal ? (
            removal.plan.dependents.length > 0 ? (
              <>
                <span className="font-medium text-danger">
                  {removal.plan.dependents.join(", ")}
                </span>{" "}
                {removal.plan.dependents.length === 1 ? "requires" : "require"} this file.
                Removing it will likely break the game.
              </>
            ) : removal.plan.from_pack ? (
              "This file came from a modpack. Removing it may break the pack."
            ) : (
              "This file brought other mods in with it."
            )
          ) : null
        }
        cancelLabel="Keep it"
        confirmLabel={
          removal && removal.orphans.length > 0
            ? `Remove ${removal.orphans.length + 1} files`
            : removal && removal.plan.dependents.length > 0
              ? "Remove anyway"
              : "Remove"
        }
        onConfirm={async () => {
          if (removal) await remove(removal.item, removal.orphans);
        }}
        onCancel={close}
      >
        {removal && removal.plan.orphans.length > 0 ? (
          <>
            <div className="text-xs font-medium text-content">
              {removal.plan.orphans.length === 1
                ? "It installed one dependency that nothing else needs"
                : `It installed ${removal.plan.orphans.length} dependencies that nothing else needs`}
            </div>
            <div className="mt-2.5 flex flex-col gap-1">
              {removal.plan.orphans.map((orphan) => {
                const checked = removal.orphans.includes(orphan.file_name);
                return (
                  <button
                    key={orphan.file_name}
                    onClick={() =>
                      setDialog({
                        ...removal,
                        orphans: checked
                          ? removal.orphans.filter((f) => f !== orphan.file_name)
                          : [...removal.orphans, orphan.file_name],
                      })
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
        plan={suggestion?.plan ?? null}
        busy={suggestBusy !== null && suggestion !== null}
        progress={null}
        onConfirm={() =>
          suggestion &&
          void installSuggestion(
            suggestion.provider,
            suggestion.project,
            true,
            suggestion.plan,
          )
        }
        onSkipDependencies={() =>
          suggestion &&
          void installSuggestion(
            suggestion.provider,
            suggestion.project,
            false,
            suggestion.plan,
          )
        }
        onCancel={close}
      />

      <ExportPackModal
        instance={dialog?.kind === "export" ? instance : null}
        onClose={close}
      />
      <EditInstanceModal
        instance={dialog?.kind === "edit" ? instance : null}
        onClose={close}
      />
      <ContextMenu menu={menu} onClose={closeMenu} />

      <ConfirmDialog
        open={dialog?.kind === "delete"}
        title={`Delete ${instance.name}?`}
        description="The whole instance folder is removed from disk, including its worlds, mods, configs, screenshots and every snapshot taken of it. This cannot be undone."
        confirmLabel="Delete instance"
        requireText={instance.name}
        onConfirm={async () => {
          close();
          await deleteInstance(instance.id);
        }}
        onCancel={close}
      />

      <SnapshotsModal
        instance={instance}
        open={dialog?.kind === "snapshots"}
        running={gameRunning}
        busyWithTask={busyWithTask}
        onClose={close}
        onRestored={refreshInstances}
      />
      <UploadModal
        open={addingFiles}
        onClose={() => {
          setAddingFiles(false);
          setAddingError(null);
        }}
        error={addingError}
        title={`Add ${tabMeta.label.toLowerCase()}`}
        subtitle={`Copied into ${instance.name}`}
        extensions={tabMeta.extensions}
        filterName={tabMeta.label}
        multiple
        busy={addingBusy}
        onConfirm={(paths) => void addFiles(paths)}
      />

      <ModpackUpgradeModal
        instance={instance}
        plan={dialog?.kind === "packUpgrade" ? dialog.plan : null}
        busy={upgradingPack}
        onUpgrade={(snapshotFirst) => void confirmPackUpgrade(snapshotFirst)}
        onPickVersion={(versionId) => void openPackUpgrade(versionId)}
        replanning={checkingPackUpgrade}
        onClose={close}
      />
      {browserDownloads.modal}
    </div>
  );
}
