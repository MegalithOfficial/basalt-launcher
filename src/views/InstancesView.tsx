import { useEffect, useMemo, useState } from "react";
import {
  ArrowDown,
  ArrowUp,
  Boxes,
  Copy,
  MoreVertical,
  FileArchive,
  Folder,
  FolderPlus,
  LayoutGrid,
  List,
  Pencil,
  Trash2,
  Plus,
  Search,
  SearchX,
  TriangleAlert,
} from "lucide-react";
import { toast } from "sonner";

import { Button, EmptyState } from "../components/ui";
import { Select } from "../components/Select";
import { Banner } from "../components/Banner";
import { ContextMenu, useContextMenu, type MenuItem } from "../components/ContextMenu";
import { CreateInstanceModal } from "../components/CreateInstanceModal";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { EditInstanceModal } from "../components/EditInstanceModal";
import { ImportPackModal } from "../components/ImportPackModal";
import { pickPackFile } from "../lib/packs";
import { cn } from "../lib/cn";
import { loaderLabel } from "../lib/loader";
import { logoSrc } from "../lib/media";
import {
  instanceTaskLabel,
  taskFraction,
  useActiveTasksByInstance,
} from "../lib/useTasks";
import { formatPlaytime, relativeTime } from "../lib/time";
import type { Instance, InstanceGroup, Task, VersionMedia } from "../lib/types";
import { PlayButton } from "../components/PlayButton";
import { useStore } from "../store";

type ViewMode = "list" | "grid";

const SORTS = ["Last played", "Most played", "Name", "Recently added"] as const;
type SortMode = (typeof SORTS)[number];

function sortInstances(list: Instance[], mode: SortMode): Instance[] {
  const sorted = [...list];
  switch (mode) {
    case "Name":
      return sorted.sort((a, b) =>
        a.name.localeCompare(b.name, undefined, { sensitivity: "base" }),
      );
    case "Recently added":
      return sorted.sort(
        (a, b) => new Date(b.created_at).getTime() - new Date(a.created_at).getTime(),
      );
    case "Most played":
      return sorted.sort((a, b) => b.playtime_secs - a.playtime_secs);
    default:
      return sorted.sort((a, b) => (b.last_played_at ?? 0) - (a.last_played_at ?? 0));
  }
}

function Artwork({ media, className }: { media: VersionMedia | null; className?: string }) {
  if (!media) {
    return (
      <div className={cn("grid place-items-center bg-surface-3 text-content-faint", className)}>
        <Boxes className="size-6" />
      </div>
    );
  }
  return (
    <Banner media={media} still className={className} />
  );
}

function ProgressStrip({ task }: { task: Task }) {
  const fraction = taskFraction(task);
  return (
    <div className="h-0.5 w-full overflow-hidden bg-surface-3">
      <div
        className={cn(
          "h-full",
          task.retry_note ? "bg-warn" : "bg-(--accent)",
          fraction == null ? "w-1/3 animate-pulse" : "transition-[width] duration-300",
        )}
        style={fraction == null ? undefined : { width: `${fraction * 100}%` }}
      />
    </div>
  );
}

function StatusLine({ instance, task }: { instance: Instance; task?: Task }) {
  if (task) {
    return task.retry_note ? (
      <span className="text-warn">Retrying</span>
    ) : (
      <span className="capitalize text-(--accent)">{instanceTaskLabel(task)}</span>
    );
  }
  const played = formatPlaytime(instance.playtime_secs);
  if (instance.last_played_at) {
    return (
      <span>
        Played {relativeTime(instance.last_played_at)}
        {played ? ` · ${played}` : ""}
      </span>
    );
  }
  return <span>Never played</span>;
}

function RowActions({
  onEdit,
  onMenu,
  floating,
}: {
  onEdit: () => void;
  onMenu: (event: React.MouseEvent) => void;
  floating?: boolean;
}) {
  const base = cn(
    "grid size-8 place-items-center rounded-lg transition-colors",
    floating
      ? "bg-black/55 text-white/80 backdrop-blur hover:bg-black/75 hover:text-white"
      : "text-content-faint hover:bg-surface-3 hover:text-content",
  );
  return (
    <>
      <button onClick={onEdit} aria-label="Edit instance" title="Edit instance" className={base}>
        <Pencil className="size-4" />
      </button>
      <button onClick={onMenu} aria-label="More actions" title="More actions" className={base}>
        <MoreVertical className="size-4" />
      </button>
    </>
  );
}

export function InstancesView() {
  const busyTasks = useActiveTasksByInstance();
  const instances = useStore((s) => s.instances);
  const organization = useStore((s) => s.instanceOrganization);
  const deleteInstance = useStore((s) => s.deleteInstance);
  const duplicateInstance = useStore((s) => s.duplicateInstance);
  const createInstanceGroup = useStore((s) => s.createInstanceGroup);
  const renameInstanceGroup = useStore((s) => s.renameInstanceGroup);
  const deleteInstanceGroup = useStore((s) => s.deleteInstanceGroup);
  const moveInstanceToGroup = useStore((s) => s.moveInstanceToGroup);
  const reorderInstanceGroups = useStore((s) => s.reorderInstanceGroups);
  const mediaMap = useStore((s) => s.media);
  const loadMedia = useStore((s) => s.loadMedia);
  const openInstance = useStore((s) => s.openInstance);
  const { menu, open: openMenu, close: closeMenu } = useContextMenu();

  const [modalOpen, setModalOpen] = useState(false);
  const [packPath, setPackPath] = useState<string | null>(null);
  const [editing, setEditing] = useState<Instance | null>(null);
  const [removing, setRemoving] = useState<Instance | null>(null);
  const [removingGroup, setRemovingGroup] = useState<InstanceGroup | null>(null);
  const [creatingGroup, setCreatingGroup] = useState(false);
  const [groupName, setGroupName] = useState("");
  const [editingGroupId, setEditingGroupId] = useState<string | null>(null);
  const [editingGroupName, setEditingGroupName] = useState("");
  const [active, setActive] = useState<string>(
    () => localStorage.getItem("instances-group") ?? "all",
  );
  const [launchError, setLaunchError] = useState<string | null>(null);
  const [dragOver, setDragOver] = useState<string | null>(null);
  const [dragging, setDragging] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [viewMode, setViewMode] = useState<ViewMode>(
    () => (localStorage.getItem("instances-view") as ViewMode) ?? "grid",
  );
  const [sort, setSort] = useState<SortMode>(() => {
    const stored = localStorage.getItem("instances-sort") as SortMode | null;
    return stored && SORTS.includes(stored) ? stored : "Last played";
  });

  const placements = useMemo(
    () => new Map(organization.placements.map((item) => [item.instance_id, item])),
    [organization.placements],
  );
  const groups = useMemo(
    () => [...organization.groups].sort((a, b) => a.sort_order - b.sort_order),
    [organization.groups],
  );
  const filtered = useMemo(() => {
    const needle = query.trim().toLocaleLowerCase();
    return needle
      ? instances.filter((instance) =>
          [
            instance.name,
            instance.version_id,
            instance.loader,
            instance.loader_version,
            instance.pack_provider,
          ].some((value) => value?.toLocaleLowerCase().includes(needle)),
        )
      : instances;
  }, [instances, query]);
  const groupOf = useMemo(() => {
    const valid = new Set(groups.map((group) => group.id));
    return (instance: Instance) => {
      const assigned = placements.get(instance.id)?.group_id ?? null;
      return assigned && valid.has(assigned) ? assigned : null;
    };
  }, [groups, placements]);

  const counts = useMemo(() => {
    const tally = new Map<string, number>();
    for (const instance of instances) {
      const key = groupOf(instance) ?? "ungrouped";
      tally.set(key, (tally.get(key) ?? 0) + 1);
    }
    return tally;
  }, [instances, groupOf]);

  const shown = useMemo(() => {
    const scoped =
      active === "all"
        ? filtered
        : filtered.filter((instance) => (groupOf(instance) ?? "ungrouped") === active);
    return sortInstances(scoped, sort);
  }, [filtered, active, groupOf, sort]);

  const instanceMenu = (instance: Instance): MenuItem[] => {
    const current = placements.get(instance.id)?.group_id ?? null;
    return [
      { label: "Open", icon: Boxes, onSelect: () => openInstance(instance.id) },
      { label: "Edit", icon: Pencil, onSelect: () => setEditing(instance) },
      { label: "Duplicate", icon: Copy, onSelect: () => void duplicate(instance) },
      ...(groups.length > 0
        ? [
            {
              label: "Move to Ungrouped",
              icon: Folder,
              separated: true,
              disabled: current === null,
              onSelect: () => void move(instance, null),
            },
            ...groups.map((group) => ({
              label: `Move to ${group.name}`,
              icon: Folder,
              disabled: current === group.id,
              onSelect: () => void move(instance, group.id),
            })),
          ]
        : []),
      {
        label: "Delete instance",
        icon: Trash2,
        danger: true,
        separated: true,
        onSelect: () => setRemoving(instance),
      },
    ];
  };

  const groupMenu = (group: InstanceGroup, index: number): MenuItem[] => [
    {
      label: "Rename",
      icon: Pencil,
      onSelect: () => {
        setEditingGroupId(group.id);
        setEditingGroupName(group.name);
      },
    },
    {
      label: "Move up",
      icon: ArrowUp,
      disabled: index === 0,
      onSelect: () => void shiftGroup(group.id, -1),
    },
    {
      label: "Move down",
      icon: ArrowDown,
      disabled: index === groups.length - 1,
      onSelect: () => void shiftGroup(group.id, 1),
    },
    {
      label: "Delete group",
      icon: Trash2,
      danger: true,
      separated: true,
      onSelect: () => setRemovingGroup(group),
    },
  ];

  const duplicate = async (instance: Instance) => {
    try {
      await duplicateInstance(instance.id);
    } catch (error) {
      toast.error(`Could not duplicate ${instance.name}`, { description: String(error) });
    }
  };

  const move = async (instance: Instance, groupId: string | null) => {
    try {
      await moveInstanceToGroup(instance.id, groupId);
    } catch (error) {
      toast.error(`Could not move ${instance.name}`, { description: String(error) });
      throw error;
    }
  };

  const submitGroup = async () => {
    const name = groupName.trim();
    if (!name) return;
    try {
      await createInstanceGroup(name);
      setGroupName("");
      setCreatingGroup(false);
    } catch (error) {
      toast.error("Could not create group", { description: String(error) });
    }
  };

  const submitRename = async (group: InstanceGroup) => {
    const name = editingGroupName.trim();
    if (!name) return;
    try {
      await renameInstanceGroup(group.id, name);
      setEditingGroupId(null);
    } catch (error) {
      toast.error("Could not rename group", { description: String(error) });
    }
  };

  const pickGroup = (id: string) => {
    setActive(id);
    localStorage.setItem("instances-group", id);
  };

  const shiftGroup = async (id: string, direction: -1 | 1) => {
    const index = groups.findIndex((group) => group.id === id);
    const target = index + direction;
    if (index < 0 || target < 0 || target >= groups.length) return;
    const ids = groups.map((group) => group.id);
    [ids[index], ids[target]] = [ids[target], ids[index]];
    try {
      await reorderInstanceGroups(ids);
    } catch (error) {
      toast.error("Could not reorder groups", { description: String(error) });
    }
  };

  const dropOnGroup = async (event: React.DragEvent, groupId: string | null) => {
    event.preventDefault();
    const instanceId = event.dataTransfer.getData("application/x-basalt-instance");
    const instance = instances.find((item) => item.id === instanceId);
    if (instance && (placements.get(instance.id)?.group_id ?? null) !== groupId) {
      await move(instance, groupId);
    }
  };

  const choosePack = async () => {
    const chosen = await pickPackFile();
    if (chosen) setPackPath(chosen);
  };

  const switchSort = (mode: SortMode) => {
    setSort(mode);
    localStorage.setItem("instances-sort", mode);
  };

  const switchView = (mode: ViewMode) => {
    setViewMode(mode);
    localStorage.setItem("instances-view", mode);
  };

  useEffect(() => {
    instances.forEach((i) => loadMedia(i.id));
  }, [instances, loadMedia]);

  useEffect(() => {
    if (active === "all" || active === "ungrouped") return;
    if (!groups.some((group) => group.id === active)) pickGroup("all");
  }, [groups, active]);

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex items-center justify-between gap-4 border-b border-border-soft px-8 py-3.5">
        <div className="flex items-baseline gap-3">
          <h1 className="font-display text-[1rem] font-semibold tracking-tight text-content">
            Instances
          </h1>
          {instances.length > 0 && (
            <span className="text-xs text-content-faint">
              {query.trim()
                ? `${filtered.length} of ${instances.length} instances`
                : `${instances.length} ${instances.length === 1 ? "instance" : "instances"}`}
            </span>
          )}
        </div>
        <div className="flex items-center gap-2">
          {instances.length > 0 && (
            <div className="relative w-56">
              <Search className="absolute left-3 top-1/2 size-3.5 -translate-y-1/2 text-content-faint" />
              <input
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                placeholder="Search instances"
                aria-label="Search instances"
                className="h-9 w-full rounded-lg border border-border-soft bg-surface-2/60 pl-9 pr-3 text-xs text-content outline-none transition-colors placeholder:text-content-faint focus:border-(--accent)"
              />
            </div>
          )}
          {instances.length > 1 && (
            <div className="w-44">
              <Select
                value={sort}
                options={[...SORTS]}
                onChange={(value) => switchSort(value as SortMode)}
              />
            </div>
          )}
          <div className="flex rounded-lg border border-border-soft bg-surface-2/60 p-0.5">
            {(
              [
                { mode: "grid", icon: LayoutGrid },
                { mode: "list", icon: List },
              ] as const
            ).map(({ mode, icon: Icon }) => (
              <button
                key={mode}
                onClick={() => switchView(mode)}
                aria-label={`${mode} view`}
                aria-pressed={viewMode === mode}
                className={cn(
                  "grid size-8 place-items-center rounded-md transition-colors",
                  viewMode === mode
                    ? "bg-surface-3 text-content"
                    : "text-content-faint hover:text-content-muted",
                )}
              >
                <Icon className="size-4" />
              </button>
            ))}
          </div>
          <button
            onClick={() => void choosePack()}
            aria-label="Import a pack file"
            title="Import a .mrpack or CurseForge pack"
            className="grid size-8 place-items-center rounded-lg border border-border-soft bg-surface-2/60 text-content-faint transition-colors hover:text-content"
          >
            <FileArchive className="size-4" />
          </button>
          <button
            onClick={() => setCreatingGroup((value) => !value)}
            aria-label="Create instance group"
            title="Create group"
            className={cn(
              "grid size-8 place-items-center rounded-lg border transition-colors",
              creatingGroup
                ? "border-(--accent)/40 bg-(--accent)/10 text-(--accent)"
                : "border-border-soft bg-surface-2/60 text-content-faint hover:text-content",
            )}
          >
            <FolderPlus className="size-4" />
          </button>
          <Button onClick={() => setModalOpen(true)}>
            <Plus className="size-4" />
            New instance
          </Button>
        </div>
      </div>

      {launchError && (
        <div className="mx-8 mt-4 flex items-start gap-2 rounded-xl border border-danger/30 bg-danger/10 px-4 py-2.5 text-sm text-danger">
          <TriangleAlert className="mt-0.5 size-4 shrink-0" />
          <span className="wrap-break-word">{launchError}</span>
        </div>
      )}

      {instances.length === 0 ? (
        <EmptyState
          icon={<Boxes className="size-6" />}
          title="No instances yet"
          description="Create an instance to choose a Minecraft version and start playing."
          action={
            <Button onClick={() => setModalOpen(true)}>
              <Plus className="size-4" />
              Create your first instance
            </Button>
          }
        />
      ) : filtered.length === 0 ? (
        <EmptyState
          icon={<SearchX className="size-6" />}
          title="No matching instances"
          description={`Nothing matches “${query.trim()}”.`}
          action={
            <Button variant="ghost" onClick={() => setQuery("")}>
              Clear search
            </Button>
          }
        />
      ) : (
        <div className="min-h-0 flex-1 overflow-y-auto px-8 pb-6">
          {(groups.length > 0 || creatingGroup) && (
            <div className="sticky top-0 z-10 -mx-8 flex flex-wrap items-center gap-1.5 bg-base/95 px-8 py-3 backdrop-blur">
              {[
                { id: "all", name: "All", group: undefined as InstanceGroup | undefined },
                ...groups.map((group) => ({ id: group.id, name: group.name, group })),
                { id: "ungrouped", name: "Ungrouped", group: undefined },
              ].map((chip) => {
                const selected = active === chip.id;
                const count =
                  chip.id === "all" ? instances.length : (counts.get(chip.id) ?? 0);
                const droppable = chip.id !== "all" && dragging !== null;
                if (chip.id === "ungrouped" && count === 0 && !droppable) return null;
                if (editingGroupId && chip.group?.id === editingGroupId) {
                  return (
                    <form
                      key={chip.id}
                      onSubmit={(event) => {
                        event.preventDefault();
                        void submitRename(chip.group!);
                      }}
                    >
                      <input
                        autoFocus
                        value={editingGroupName}
                        onChange={(event) => setEditingGroupName(event.target.value)}
                        onBlur={() => setEditingGroupId(null)}
                        maxLength={64}
                        className="h-8 w-40 rounded-lg border border-(--accent)/50 bg-surface-2 px-2.5 text-xs font-medium text-content outline-none"
                      />
                    </form>
                  );
                }
                return (
                  <button
                    key={chip.id}
                    onClick={() => pickGroup(chip.id)}
                    onContextMenu={(event) =>
                      chip.group &&
                      openMenu(
                        event,
                        groupMenu(chip.group, groups.findIndex((g) => g.id === chip.group!.id)),
                        chip.name,
                      )
                    }
                    onDragOver={(event) => {
                      if (!droppable) return;
                      event.preventDefault();
                      if (dragOver !== chip.id) setDragOver(chip.id);
                    }}
                    onDragLeave={() =>
                      setDragOver((current) => (current === chip.id ? null : current))
                    }
                    onDrop={(event) => {
                      setDragOver(null);
                      if (!droppable) return;
                      void dropOnGroup(event, chip.id === "ungrouped" ? null : chip.id);
                    }}
                    className={cn(
                      "inline-flex h-8 shrink-0 items-center gap-1.5 rounded-lg border px-3 text-xs font-medium transition-colors",
                      dragOver === chip.id && droppable
                        ? "border-(--accent) bg-(--accent)/15 text-content"
                        : selected
                          ? "border-(--accent)/40 bg-(--accent)/10 text-content"
                          : "border-border-soft bg-surface-2/60 text-content-muted hover:border-border hover:text-content",
                    )}
                  >
                    {chip.group && <Folder className="size-3.5 shrink-0" />}
                    <span className="max-w-40 truncate">{chip.name}</span>
                    <span className="tabular-nums text-content-faint">{count}</span>
                  </button>
                );
              })}

              {creatingGroup ? (
                <form
                  onSubmit={(event) => {
                    event.preventDefault();
                    void submitGroup();
                  }}
                >
                  <input
                    autoFocus
                    value={groupName}
                    onChange={(event) => setGroupName(event.target.value)}
                    onBlur={() => {
                      setCreatingGroup(false);
                      setGroupName("");
                    }}
                    onKeyDown={(event) => {
                      if (event.key === "Escape") {
                        setCreatingGroup(false);
                        setGroupName("");
                      }
                    }}
                    placeholder="Group name"
                    maxLength={64}
                    className="h-8 w-40 rounded-lg border border-(--accent)/50 bg-surface-2 px-2.5 text-xs font-medium text-content outline-none placeholder:text-content-faint"
                  />
                </form>
              ) : (
                <button
                  onClick={() => setCreatingGroup(true)}
                  className="grid size-8 shrink-0 place-items-center rounded-lg border border-dashed border-border text-content-faint transition-colors hover:border-(--accent)/50 hover:text-content"
                  aria-label="New group"
                  title="New group"
                >
                  <Plus className="size-3.5" />
                </button>
              )}
            </div>
          )}

          <div className={cn(groups.length > 0 || creatingGroup ? "pt-1" : "pt-6")}>
            {viewMode === "list" ? (
              <div className="flex flex-col gap-2">
                {shown.map((it) => {
                  const task = busyTasks.get(it.id);
                  return (
                    <div
                      key={it.id}
                      draggable
                      onDragStart={(event) => {
                        event.dataTransfer.effectAllowed = "move";
                        event.dataTransfer.setData("application/x-basalt-instance", it.id);
                        setDragging(it.id);
                      }}
                      onDragEnd={() => {
                        setDragging(null);
                        setDragOver(null);
                      }}
                      onContextMenu={(event) => openMenu(event, instanceMenu(it), it.name)}
                      className={cn(
                        "flex items-center gap-4 overflow-hidden rounded-2xl border border-border-soft bg-surface-2/60 p-3 transition-colors hover:border-border",
                        dragging === it.id && "opacity-40",
                      )}
                    >
                      <button
                        onClick={() => openInstance(it.id)}
                        aria-label={`Open ${it.name}`}
                        className="relative size-16 shrink-0 overflow-hidden rounded-xl"
                      >
                        <Artwork media={mediaMap[it.id] ?? null} className="size-full" />
                        {logoSrc(it.logo) && (
                          <img src={logoSrc(it.logo)!} alt="" draggable={false} className="absolute inset-0 size-full object-cover" />
                        )}
                      </button>
                      <button onClick={() => openInstance(it.id)} className="min-w-0 flex-1 text-left">
                        <div className="truncate font-display font-semibold text-content">{it.name}</div>
                        <div className="mt-1 flex flex-wrap items-center gap-1.5">
                          <span className="rounded border border-border-soft bg-surface-2 px-1.5 py-0.5 font-pixel text-[10px] text-content-muted">{it.version_id}</span>
                          {it.loader && <span className="rounded border border-border-soft bg-surface-2 px-1.5 py-0.5 text-[10px] font-medium text-content-muted">{loaderLabel(it)}</span>}
                          <span className="text-[11px] text-content-faint"><StatusLine instance={it} task={task} /></span>
                        </div>
                      </button>
                      <div className="flex shrink-0 items-center gap-1">
                        <PlayButton instance={it} onError={setLaunchError} />
                        <RowActions
                          onEdit={() => setEditing(it)}
                          onMenu={(event) => openMenu(event, instanceMenu(it), it.name)}
                        />
                      </div>
                    </div>
                  );
                })}
              </div>
            ) : (
              <div className="grid auto-rows-min grid-cols-[repeat(auto-fill,minmax(17rem,1fr))] content-start gap-4">
                {shown.map((it) => {
                  const task = busyTasks.get(it.id);
                  return (
                    <div
                      key={it.id}
                      draggable
                      onDragStart={(event) => {
                        event.dataTransfer.effectAllowed = "move";
                        event.dataTransfer.setData("application/x-basalt-instance", it.id);
                        setDragging(it.id);
                      }}
                      onDragEnd={() => {
                        setDragging(null);
                        setDragOver(null);
                      }}
                      onContextMenu={(event) => openMenu(event, instanceMenu(it), it.name)}
                      className={cn(
                        "group flex flex-col overflow-hidden rounded-2xl border border-border-soft bg-surface-2/60 transition-colors hover:border-border",
                        dragging === it.id && "opacity-40",
                      )}
                    >
                      <div className="relative aspect-16/10 w-full overflow-hidden">
                        <Artwork media={mediaMap[it.id] ?? null} className="absolute inset-0 size-full transition-transform duration-500 group-hover:scale-105" />
                        <div className="pointer-events-none absolute inset-x-0 bottom-0 h-3/5 bg-linear-to-t from-black/90 via-black/45 to-transparent" />
                        <button onClick={() => openInstance(it.id)} aria-label={`Open ${it.name}`} className="absolute inset-0" />
                        {logoSrc(it.logo) && <img src={logoSrc(it.logo)!} alt="" draggable={false} className="pointer-events-none absolute left-3 top-3 size-10 rounded-xl border border-white/15 bg-black/40 object-cover shadow-lg backdrop-blur" />}
                        <div className="pointer-events-none absolute inset-x-3 bottom-3">
                          <div className="truncate font-display text-[1rem] font-semibold text-white drop-shadow">{it.name}</div>
                          <div className="mt-1.5 flex flex-wrap items-center gap-1.5">
                            <span className="rounded bg-black/55 px-1.5 py-0.5 font-pixel text-[10px] text-white/80 backdrop-blur">{it.version_id}</span>
                            {it.loader && <span className="rounded bg-black/55 px-1.5 py-0.5 text-[10px] font-medium text-white/80 backdrop-blur">{loaderLabel(it)}</span>}
                          </div>
                        </div>
                        <div className="absolute right-2 top-2 flex items-center gap-1 opacity-0 transition-opacity focus-within:opacity-100 group-hover:opacity-100">
                          <RowActions
                            floating
                            onEdit={() => setEditing(it)}
                            onMenu={(event) => openMenu(event, instanceMenu(it), it.name)}
                          />
                        </div>
                      </div>
                      {task && <ProgressStrip task={task} />}
                      <div className="flex items-center justify-between gap-2 px-3 py-2.5">
                        <span className="min-w-0 truncate text-[11px] text-content-faint"><StatusLine instance={it} task={task} /></span>
                        <PlayButton instance={it} compact onError={setLaunchError} />
                      </div>
                    </div>
                  );
                })}
                <button
                  onClick={() => setModalOpen(true)}
                  className="group flex min-h-52 flex-col items-center justify-center gap-3 rounded-2xl border border-dashed border-border text-content-faint transition-colors hover:border-(--accent)/50 hover:bg-surface-2/40 hover:text-content"
                >
                  <span className="grid size-11 place-items-center rounded-full border border-border-soft bg-surface-2 transition-colors group-hover:border-(--accent)/40"><Plus className="size-5" /></span>
                  <span className="text-xs font-medium">New instance</span>
                </button>
              </div>
            )}

            {shown.length === 0 && (
              <div className="grid min-h-40 place-items-center text-sm text-content-faint">
                Nothing in this group yet. Drag an instance onto its chip to file it here.
              </div>
            )}
          </div>
        </div>
      )}

      <ContextMenu menu={menu} onClose={closeMenu} />

      <CreateInstanceModal
        open={modalOpen}
        onClose={() => setModalOpen(false)}
        onCreated={() => {}}
        onImportFile={() => void choosePack()}
      />
      <ImportPackModal path={packPath} onClose={() => setPackPath(null)} />
      <EditInstanceModal instance={editing} onClose={() => setEditing(null)} />

      <ConfirmDialog
        open={!!removingGroup}
        title={removingGroup ? `Delete ${removingGroup.name}?` : ""}
        description="The instances in this group will move to Ungrouped. No instance files will be deleted."
        confirmLabel="Delete group"
        onConfirm={async () => {
          if (!removingGroup) return;
          try {
            await deleteInstanceGroup(removingGroup.id);
          } catch (error) {
            toast.error("Could not delete group", { description: String(error) });
          }
          setRemovingGroup(null);
        }}
        onCancel={() => setRemovingGroup(null)}
      />

      <ConfirmDialog
        open={!!removing}
        title={removing ? `Delete ${removing.name}?` : ""}
        description="The whole instance folder is removed from disk, including its worlds, mods, configs and screenshots. This cannot be undone."
        confirmLabel="Delete instance"
        requireText={removing?.name}
        onConfirm={async () => {
          if (removing) await deleteInstance(removing.id);
          setRemoving(null);
        }}
        onCancel={() => setRemoving(null)}
      />
    </div>
  );
}
