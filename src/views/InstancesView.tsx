import { useEffect, useMemo, useState } from "react";
import {
  Boxes,
  FileArchive,
  LayoutGrid,
  List,
  Pencil,
  Trash2,
  Plus,
  TriangleAlert,
} from "lucide-react";

import { Button, EmptyState } from "../components/ui";
import { Select } from "../components/Select";
import { Banner } from "../components/Banner";
import { CreateInstanceModal } from "../components/CreateInstanceModal";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { EditInstanceModal } from "../components/EditInstanceModal";
import { ImportPackModal } from "../components/ImportPackModal";
import { pickPackFile } from "../lib/packs";
import { cn } from "../lib/cn";
import { loaderLabel } from "../lib/loader";
import { logoSrc } from "../lib/media";
import { taskFraction, useActiveTasksByInstance } from "../lib/useTasks";
import { formatPlaytime, relativeTime } from "../lib/time";
import type { Instance, Task, VersionMedia } from "../lib/types";
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
      <span className="capitalize text-(--accent)">{task.stage}</span>
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
  onDelete,
  floating,
}: {
  onEdit: () => void;
  onDelete: () => void;
  floating?: boolean;
}) {
  const base = "grid size-8 place-items-center rounded-lg transition-colors";
  return (
    <>
      <button
        onClick={onEdit}
        title="Edit instance"
        className={cn(
          base,
          floating
            ? "bg-black/55 text-white/80 backdrop-blur hover:bg-black/75 hover:text-white"
            : "text-content-faint hover:bg-surface-3 hover:text-content",
        )}
      >
        <Pencil className="size-4" />
      </button>
      <button
        onClick={onDelete}
        title="Delete instance"
        className={cn(
          base,
          floating
            ? "bg-black/55 text-white/80 backdrop-blur hover:bg-danger hover:text-white"
            : "text-content-faint hover:bg-danger/15 hover:text-danger",
        )}
      >
        <Trash2 className="size-4" />
      </button>
    </>
  );
}

export function InstancesView() {
  const busyTasks = useActiveTasksByInstance();
  const instances = useStore((s) => s.instances);
  const deleteInstance = useStore((s) => s.deleteInstance);
  const mediaMap = useStore((s) => s.media);
  const loadMedia = useStore((s) => s.loadMedia);
  const openInstance = useStore((s) => s.openInstance);

  const [modalOpen, setModalOpen] = useState(false);
  const [packPath, setPackPath] = useState<string | null>(null);
  const [editing, setEditing] = useState<Instance | null>(null);
  const [removing, setRemoving] = useState<Instance | null>(null);
  const [launchError, setLaunchError] = useState<string | null>(null);
  const [viewMode, setViewMode] = useState<ViewMode>(
    () => (localStorage.getItem("instances-view") as ViewMode) ?? "grid",
  );
  const [sort, setSort] = useState<SortMode>(() => {
    const stored = localStorage.getItem("instances-sort") as SortMode | null;
    return stored && SORTS.includes(stored) ? stored : "Last played";
  });

  const ordered = useMemo(() => sortInstances(instances, sort), [instances, sort]);

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

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex items-center justify-between gap-4 border-b border-border-soft px-8 py-3.5">
        <div className="flex items-baseline gap-3">
          <h1 className="font-display text-[1rem] font-semibold tracking-tight text-content">
            Instances
          </h1>
          {instances.length > 0 && (
            <span className="text-xs text-content-faint">
              {instances.length} {instances.length === 1 ? "instance" : "instances"}
            </span>
          )}
        </div>
        <div className="flex items-center gap-2">
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
      ) : viewMode === "list" ? (
        <div className="min-h-0 flex-1 overflow-y-auto px-8 py-6">
          <div className="flex flex-col gap-2">
            {ordered.map((it) => {
              const task = busyTasks.get(it.id);
              return (
                <div
                  key={it.id}
                  className="flex items-center gap-4 overflow-hidden rounded-2xl border border-border-soft bg-surface-2/60 p-3 transition-colors hover:border-border"
                >
                  <button
                    onClick={() => openInstance(it.id)}
                    aria-label={`Open ${it.name}`}
                    className="relative size-16 shrink-0 overflow-hidden rounded-xl"
                  >
                    <Artwork media={mediaMap[it.id] ?? null} className="size-full" />
                    {logoSrc(it.logo) && (
                      <img
                        src={logoSrc(it.logo)!}
                        alt=""
                        draggable={false}
                        className="absolute inset-0 size-full object-cover"
                      />
                    )}
                  </button>

                  <button
                    onClick={() => openInstance(it.id)}
                    className="min-w-0 flex-1 text-left"
                  >
                    <div className="truncate font-display font-semibold text-content">
                      {it.name}
                    </div>
                    <div className="mt-1 flex flex-wrap items-center gap-1.5">
                      <span className="rounded border border-border-soft bg-surface-2 px-1.5 py-0.5 font-pixel text-[10px] text-content-muted">
                        {it.version_id}
                      </span>
                      {it.loader && (
                        <span className="rounded border border-border-soft bg-surface-2 px-1.5 py-0.5 text-[10px] font-medium text-content-muted">
                          {loaderLabel(it)}
                        </span>
                      )}
                      <span className="text-[11px] text-content-faint">
                        <StatusLine instance={it} task={task} />
                      </span>
                    </div>
                  </button>

                  <div className="flex shrink-0 items-center gap-1">
                    <PlayButton instance={it} onError={setLaunchError} />
                    <RowActions
                      onEdit={() => setEditing(it)}
                      onDelete={() => setRemoving(it)}
                    />
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      ) : (
        <div className="min-h-0 flex-1 overflow-y-auto px-8 py-6">
          <div className="grid auto-rows-min grid-cols-[repeat(auto-fill,minmax(17rem,1fr))] content-start gap-4">
            {ordered.map((it) => {
              const task = busyTasks.get(it.id);
              return (
                <div
                  key={it.id}
                  className="group flex flex-col overflow-hidden rounded-2xl border border-border-soft bg-surface-2/60 transition-colors hover:border-border"
                >
                  <div className="relative aspect-16/10 w-full overflow-hidden">
                    <Artwork
                      media={mediaMap[it.id] ?? null}
                      className="absolute inset-0 size-full transition-transform duration-500 group-hover:scale-105"
                    />
                    <div className="pointer-events-none absolute inset-x-0 bottom-0 h-3/5 bg-linear-to-t from-black/90 via-black/45 to-transparent" />

                    <button
                      onClick={() => openInstance(it.id)}
                      aria-label={`Open ${it.name}`}
                      className="absolute inset-0"
                    />

                    {logoSrc(it.logo) && (
                      <img
                        src={logoSrc(it.logo)!}
                        alt=""
                        draggable={false}
                        className="pointer-events-none absolute left-3 top-3 size-10 rounded-xl border border-white/15 bg-black/40 object-cover shadow-lg backdrop-blur"
                      />
                    )}

                    <div className="pointer-events-none absolute inset-x-3 bottom-3">
                      <div className="truncate font-display text-[1rem] font-semibold text-white drop-shadow">
                        {it.name}
                      </div>
                      <div className="mt-1.5 flex flex-wrap items-center gap-1.5">
                        <span className="rounded bg-black/55 px-1.5 py-0.5 font-pixel text-[10px] text-white/80 backdrop-blur">
                          {it.version_id}
                        </span>
                        {it.loader && (
                          <span className="rounded bg-black/55 px-1.5 py-0.5 text-[10px] font-medium text-white/80 backdrop-blur">
                            {loaderLabel(it)}
                          </span>
                        )}
                      </div>
                    </div>

                    <div className="absolute right-2 top-2 flex items-center gap-1 opacity-0 transition-opacity focus-within:opacity-100 group-hover:opacity-100">
                      <RowActions
                        floating
                        onEdit={() => setEditing(it)}
                        onDelete={() => setRemoving(it)}
                      />
                    </div>
                  </div>

                  {task && <ProgressStrip task={task} />}

                  <div className="flex items-center justify-between gap-2 px-3 py-2.5">
                    <span className="min-w-0 truncate text-[11px] text-content-faint">
                      <StatusLine instance={it} task={task} />
                    </span>
                    <PlayButton instance={it} compact onError={setLaunchError} />
                  </div>
                </div>
              );
            })}

            <button
              onClick={() => setModalOpen(true)}
              className="group flex min-h-52 flex-col items-center justify-center gap-3 rounded-2xl border border-dashed border-border text-content-faint transition-colors hover:border-(--accent)/50 hover:bg-surface-2/40 hover:text-content"
            >
              <span className="grid size-11 place-items-center rounded-full border border-border-soft bg-surface-2 transition-colors group-hover:border-(--accent)/40">
                <Plus className="size-5" />
              </span>
              <span className="text-xs font-medium">New instance</span>
            </button>
          </div>
        </div>
      )}

      <CreateInstanceModal
        open={modalOpen}
        onClose={() => setModalOpen(false)}
        onCreated={() => {}}
        onImportFile={() => void choosePack()}
      />
      <ImportPackModal path={packPath} onClose={() => setPackPath(null)} />
      <EditInstanceModal instance={editing} onClose={() => setEditing(null)} />

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
