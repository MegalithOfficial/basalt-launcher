import { useCallback, useEffect, useState } from "react";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import {
  ArrowLeft,
  FileBox,
  Loader2,
  Package,
  Pencil,
  Plus,
  Trash2,
} from "lucide-react";

import { EditInstanceModal } from "../components/EditInstanceModal";
import { PlayButton } from "../components/PlayButton";
import { cn } from "../lib/cn";
import { api } from "../lib/api";
import { loaderLabel } from "../lib/loader";
import { mediaSrc } from "../lib/media";
import { formatPlaytime, relativeTime } from "../lib/time";
import type { ContentItem, ContentKind } from "../lib/types";
import { useStore } from "../store";

const TABS: Array<{ kind: ContentKind; label: string; extensions: string[] }> = [
  { kind: "mods", label: "Mods", extensions: ["jar"] },
  { kind: "resourcepacks", label: "Resource Packs", extensions: ["zip"] },
  { kind: "shaderpacks", label: "Shaders", extensions: ["zip"] },
];

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function Toggle({ on, onClick }: { on: boolean; onClick: () => void }) {
  return (
    <button
      onClick={onClick}
      aria-label={on ? "Disable" : "Enable"}
      className={cn(
        "relative h-5 w-9 shrink-0 rounded-full transition-colors duration-300",
        on ? "bg-[var(--accent)]" : "bg-surface-3",
      )}
    >
      <span
        className={cn(
          "absolute top-0.5 size-4 rounded-full bg-white shadow transition-transform duration-300",
          on ? "translate-x-[18px]" : "translate-x-0.5",
        )}
      />
    </button>
  );
}

export function InstanceView() {
  const detailId = useStore((s) => s.detailInstanceId);
  const instance = useStore((s) => s.instances.find((i) => i.id === s.detailInstanceId));
  const media = useStore((s) => (detailId ? (s.media[detailId] ?? null) : null));
  const setView = useStore((s) => s.setView);

  const [tab, setTab] = useState<ContentKind>("mods");
  const [items, setItems] = useState<ContentItem[]>([]);
  const [loading, setLoading] = useState(false);
  const [editOpen, setEditOpen] = useState(false);
  const [launchError, setLaunchError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    if (!instance) return;
    setLoading(true);
    try {
      setItems(await api.listInstanceContent(instance.id, tab));
    } catch {
      setItems([]);
    } finally {
      setLoading(false);
    }
  }, [instance?.id, tab]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  if (!instance) {
    return (
      <div className="grid flex-1 place-items-center text-sm text-content-muted">
        Instance not found.
      </div>
    );
  }

  const tabMeta = TABS.find((t) => t.kind === tab)!;

  const addContent = async () => {
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
    await api.toggleInstanceContent(instance.id, tab, item.file_name);
    await refresh();
  };

  const remove = async (item: ContentItem) => {
    await api.deleteInstanceContent(instance.id, tab, item.file_name);
    await refresh();
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="relative h-40 shrink-0 overflow-hidden">
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
        <div className="absolute inset-0 bg-gradient-to-t from-base via-black/50 to-black/20" />

        <button
          onClick={() => setView("instances")}
          className="absolute left-4 top-4 grid size-9 place-items-center rounded-full border border-white/10 bg-black/50 text-white/80 backdrop-blur transition-colors hover:bg-black/70 hover:text-white"
        >
          <ArrowLeft className="size-4" />
        </button>

        <div className="absolute inset-x-0 bottom-0 flex items-end justify-between gap-4 px-6 pb-4">
          <div className="min-w-0">
            <h1 className="truncate font-display text-3xl font-bold tracking-tight text-white drop-shadow">
              {instance.name}
            </h1>
            <div className="mt-1.5 flex items-center gap-2 text-[11px] text-white/60">
              <span className="rounded-md bg-black/50 px-2 py-0.5 font-pixel tracking-wider backdrop-blur">
                {instance.version_id} · {loaderLabel(instance).toUpperCase()}
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
          <div className="flex shrink-0 items-center gap-2">
            <PlayButton instance={instance} onError={setLaunchError} />
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
          {TABS.map((t) => (
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
              {t.label}
              {tab === t.kind && (
                <span className="absolute inset-x-2 -bottom-px h-0.5 rounded-full bg-[var(--accent)] transition-colors duration-500" />
              )}
            </button>
          ))}
        </div>
        <button
          onClick={addContent}
          className="mb-2 inline-flex items-center gap-1.5 rounded-lg px-3 py-2 text-xs font-semibold text-black shadow-md shadow-[var(--accent-glow)] transition-all [background:linear-gradient(to_bottom,var(--accent),var(--accent-deep))] hover:[background:linear-gradient(to_bottom,var(--accent-bright),var(--accent))]"
        >
          <Plus className="size-3.5" />
          Add content
        </button>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto p-6">
        {loading ? (
          <div className="flex items-center justify-center gap-2 py-12 text-sm text-content-muted">
            <Loader2 className="size-4 animate-spin" />
            Loading
          </div>
        ) : items.length === 0 ? (
          <div className="flex flex-col items-center gap-3 py-14 text-center">
            <div className="grid size-12 place-items-center rounded-2xl border border-border-soft bg-surface-2 text-content-faint">
              <Package className="size-6" />
            </div>
            <div className="text-sm font-medium text-content-muted">
              No {tabMeta.label.toLowerCase()} yet
            </div>
            <p className="max-w-sm text-xs text-content-faint">
              Drop files into this instance with Add content. Searching Modrinth and
              CurseForge from here is coming later.
            </p>
          </div>
        ) : (
          <div className="flex flex-col gap-1.5">
            {items.map((item) => (
              <div
                key={item.file_name}
                className={cn(
                  "flex items-center gap-3 rounded-xl border border-border-soft bg-surface-2/70 px-4 py-2.5 transition-opacity",
                  !item.enabled && "opacity-55",
                )}
              >
                <div className="grid size-8 shrink-0 place-items-center rounded-lg bg-surface-3 text-content-faint">
                  <FileBox className="size-4" />
                </div>
                <div className="min-w-0 flex-1">
                  <div className="truncate text-sm font-medium text-content">
                    {item.file_name}
                  </div>
                  <div className="text-[11px] text-content-faint">
                    {formatSize(item.size)}
                    {!item.enabled && " · disabled"}
                  </div>
                </div>
                <Toggle on={item.enabled} onClick={() => toggle(item)} />
                <button
                  onClick={() => remove(item)}
                  aria-label="Delete file"
                  className="grid size-8 place-items-center rounded-lg text-content-faint transition-colors hover:bg-danger/15 hover:text-danger"
                >
                  <Trash2 className="size-4" />
                </button>
              </div>
            ))}
          </div>
        )}
      </div>

      <EditInstanceModal
        instance={editOpen ? instance : null}
        onClose={() => setEditOpen(false)}
      />
    </div>
  );
}
