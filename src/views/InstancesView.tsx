import { useEffect, useState } from "react";
import {
  Boxes,
  LayoutGrid,
  List,
  Pencil,
  Plus,
  Trash2,
  TriangleAlert,
} from "lucide-react";

import { Button, EmptyState, PageHeader } from "../components/ui";
import { CreateInstanceModal } from "../components/CreateInstanceModal";
import { EditInstanceModal } from "../components/EditInstanceModal";
import { cn } from "../lib/cn";
import { loaderLabel } from "../lib/loader";
import { mediaSrc } from "../lib/media";
import { formatPlaytime, relativeTime } from "../lib/time";
import type { Instance } from "../lib/types";
import { PlayButton } from "../components/PlayButton";
import { useStore } from "../store";

type ViewMode = "list" | "grid";

export function InstancesView() {
  const instances = useStore((s) => s.instances);
  const deleteInstance = useStore((s) => s.deleteInstance);
  const mediaMap = useStore((s) => s.media);
  const loadMedia = useStore((s) => s.loadMedia);
  const openInstance = useStore((s) => s.openInstance);

  const [modalOpen, setModalOpen] = useState(false);
  const [editing, setEditing] = useState<Instance | null>(null);
  const [launchError, setLaunchError] = useState<string | null>(null);
  const [viewMode, setViewMode] = useState<ViewMode>(
    () => (localStorage.getItem("instances-view") as ViewMode) ?? "list",
  );

  const switchView = (mode: ViewMode) => {
    setViewMode(mode);
    localStorage.setItem("instances-view", mode);
  };

  useEffect(() => {
    instances.forEach((i) => loadMedia(i.id));
  }, [instances, loadMedia]);

  return (
    <div className="flex flex-1 flex-col">
      <PageHeader
        title="Instances"
        subtitle="Each instance is a version with its own game directory."
        actions={
          <div className="flex items-center gap-2">
            <div className="flex rounded-lg border border-border bg-surface-2 p-0.5">
              {(
                [
                  { mode: "list", icon: List },
                  { mode: "grid", icon: LayoutGrid },
                ] as const
              ).map(({ mode, icon: Icon }) => (
                <button
                  key={mode}
                  onClick={() => switchView(mode)}
                  aria-label={`${mode} view`}
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
            <Button onClick={() => setModalOpen(true)}>
              <Plus className="size-4" />
              New instance
            </Button>
          </div>
        }
      />

      {launchError && (
        <div className="mx-6 mt-4 flex items-start gap-2 rounded-xl border border-danger/30 bg-danger/10 px-4 py-2.5 text-sm text-danger">
          <TriangleAlert className="mt-0.5 size-4 shrink-0" />
          <span className="break-words">{launchError}</span>
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
        <div className="flex flex-1 flex-col gap-2 overflow-y-auto p-6">
          {instances.map((it) => (
            <div
              key={it.id}
              className="flex items-center gap-4 rounded-xl border border-border bg-surface-2 px-4 py-3"
            >
              {mediaMap[it.id] ? (
                <img
                  src={mediaSrc(mediaMap[it.id]!)}
                  className="size-10 shrink-0 cursor-pointer rounded-lg object-cover"
                  onClick={() => openInstance(it.id)}
                  draggable={false}
                />
              ) : (
                <div
                  onClick={() => openInstance(it.id)}
                  className="grid size-10 shrink-0 cursor-pointer place-items-center rounded-lg bg-surface-3 text-content-muted"
                >
                  <Boxes className="size-5" />
                </div>
              )}
              <div
                className="min-w-0 flex-1 cursor-pointer"
                onClick={() => openInstance(it.id)}
              >
                <div className="truncate font-display font-semibold text-content">{it.name}</div>
                <div className="truncate text-xs text-content-muted">
                  {it.version_id}
                  {it.loader && ` · ${loaderLabel(it)}`}
                  {it.last_played_at && ` · played ${relativeTime(it.last_played_at)}`}
                  {formatPlaytime(it.playtime_secs) && ` · ${formatPlaytime(it.playtime_secs)}`}
                </div>
              </div>

              <PlayButton instance={it} onError={setLaunchError} />
              <button
                onClick={() => setEditing(it)}
                title="Edit instance"
                className="grid size-8 place-items-center rounded-lg text-content-faint transition-colors hover:bg-surface-3 hover:text-content"
              >
                <Pencil className="size-4" />
              </button>
              <button
                onClick={() => deleteInstance(it.id)}
                title="Delete instance"
                className="grid size-8 place-items-center rounded-lg text-content-faint transition-colors hover:bg-danger/15 hover:text-danger"
              >
                <Trash2 className="size-4" />
              </button>
            </div>
          ))}
        </div>
      ) : (
        <div className="grid flex-1 auto-rows-min grid-cols-[repeat(auto-fill,minmax(250px,1fr))] content-start gap-4 overflow-y-auto p-6">
          {instances.map((it) => {
            const media = mediaMap[it.id] ?? null;
            return (
              <div
                key={it.id}
                className="group overflow-hidden rounded-2xl border border-border bg-surface-2 transition-colors hover:border-content-faint/30"
              >
                <div
                  className="relative aspect-[16/9] cursor-pointer"
                  onClick={() => openInstance(it.id)}
                >
                  {media ? (
                    <img
                      src={mediaSrc(media)}
                      className={cn(
                        "absolute inset-0 h-full w-full object-cover transition-transform duration-300 group-hover:scale-[1.03]",
                        !media.local && "[image-rendering:pixelated]",
                      )}
                      draggable={false}
                    />
                  ) : (
                    <div className="grid h-full w-full place-items-center bg-surface-3 text-content-faint">
                      <Boxes className="size-7" />
                    </div>
                  )}
                  <div className="absolute inset-x-0 bottom-0 bg-gradient-to-t from-black/85 to-transparent px-4 pb-2.5 pt-8">
                    <div className="truncate font-display font-semibold text-white">{it.name}</div>
                    <div className="truncate font-pixel text-[10px] text-white/60">
                      {it.version_id}
                      {it.loader && ` · ${loaderLabel(it)}`}
                    </div>
                  </div>
                </div>
                <div className="flex items-center justify-between gap-2 px-3 py-2.5">
                  <span className="truncate text-[11px] text-content-faint">
                    {it.last_played_at
                      ? `Played ${relativeTime(it.last_played_at)}`
                      : "Never played"}
                  </span>
                  <div className="flex shrink-0 items-center gap-1">
                    <PlayButton instance={it} compact onError={setLaunchError} />
                    <button
                      onClick={() => setEditing(it)}
                      title="Edit instance"
                      className="grid size-8 place-items-center rounded-lg text-content-faint transition-colors hover:bg-surface-3 hover:text-content"
                    >
                      <Pencil className="size-4" />
                    </button>
                    <button
                      onClick={() => deleteInstance(it.id)}
                      title="Delete instance"
                      className="grid size-8 place-items-center rounded-lg text-content-faint transition-colors hover:bg-danger/15 hover:text-danger"
                    >
                      <Trash2 className="size-4" />
                    </button>
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      )}

      <CreateInstanceModal open={modalOpen} onClose={() => setModalOpen(false)} onCreated={() => {}} />
      <EditInstanceModal instance={editing} onClose={() => setEditing(null)} />
    </div>
  );
}
