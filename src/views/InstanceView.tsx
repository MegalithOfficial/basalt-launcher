import { useCallback, useEffect, useState } from "react";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import {
  ArrowLeft,
  ArrowUpCircle,
  FileBox,
  Loader2,
  Package,
  Pencil,
  Plus,
  RefreshCw,
  Search,
  Trash2,
  TriangleAlert,
} from "lucide-react";

import { EditInstanceModal } from "../components/EditInstanceModal";
import { PlayButton } from "../components/PlayButton";
import { cn } from "../lib/cn";
import { api } from "../lib/api";
import { loaderLabel } from "../lib/loader";
import { logoSrc, mediaSrc } from "../lib/media";
import { formatPlaytime, relativeTime } from "../lib/time";
import type { ContentItem, ContentKind, ContentUpdate } from "../lib/types";
import { useActiveProjectIds } from "../lib/useTasks";
import { useStore } from "../store";

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

const SCHEMATIC_MOD_MARKERS = ["litematica", "worldedit", "schematica", "axiom", "schematic"];

const NO_UPDATES: ContentUpdate[] = [];

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
  const setView = useStore((s) => s.setView);
  const openSearch = useStore((s) => s.openSearch);
  const openProject = useStore((s) => s.openProject);
  const refreshContentSources = useStore((s) => s.refreshContentSources);
  const refreshUpdates = useStore((s) => s.refreshUpdates);
  const applyUpdate = useStore((s) => s.applyUpdate);
  const beginToastBatch = useStore((s) => s.beginToastBatch);
  const endToastBatch = useStore((s) => s.endToastBatch);
  const storedUpdates = useStore((s) => (detailId ? s.updates[detailId] : undefined));
  const updates = storedUpdates ?? NO_UPDATES;
  const activeProjects = useActiveProjectIds();

  const [tab, setTab] = useState<ContentKind>("mods");
  const [items, setItems] = useState<ContentItem[]>([]);
  const [loading, setLoading] = useState(false);
  const [editOpen, setEditOpen] = useState(false);
  const [launchError, setLaunchError] = useState<string | null>(null);
  const [hasSchematicMod, setHasSchematicMod] = useState(false);
  const [filter, setFilter] = useState("");
  const [checkingUpdates, setCheckingUpdates] = useState(false);
  const [updatingAll, setUpdatingAll] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState<{
    item: ContentItem;
    dependents: string[];
  } | null>(null);

  const refresh = useCallback(
    async (reconcile = false) => {
      if (!instance) return;
      setLoading(true);
      try {
        setItems(await api.listInstanceContent(instance.id, tab, reconcile));
        void refreshContentSources(instance.id, tab);
      } catch {
        setItems([]);
      } finally {
        setLoading(false);
      }
    },
    [instance?.id, tab, refreshContentSources],
  );

  useEffect(() => {
    void refresh(true);
  }, [refresh]);

  useEffect(() => {
    setTab(instance?.loader ? "mods" : "resourcepacks");
  }, [instance?.id]);

  useEffect(() => {
    if (instance) void refreshUpdates(instance.id);
  }, [instance?.id, refreshUpdates]);

  useEffect(() => {
    if (!instance) return;
    let live = true;
    api
      .listInstanceContent(instance.id, "mods")
      .then((mods) => {
        if (!live) return;
        const found = mods.some((m) =>
          SCHEMATIC_MOD_MARKERS.some((marker) =>
            m.file_name.toLowerCase().includes(marker),
          ),
        );
        setHasSchematicMod(found);
      })
      .catch(() => live && setHasSchematicMod(false));
    return () => {
      live = false;
    };
  }, [instance?.id, items]);

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
  const allTabs = hasSchematicMod ? [...baseTabs, SCHEMATICS_TAB] : baseTabs;
  const tabMeta = allTabs.find((t) => t.kind === tab) ?? allTabs[0];
  const tabUpdates = updates.filter((u) => u.kind === tab);
  const query = filter.trim().toLowerCase();
  const shownItems = query
    ? items.filter(
        (i) =>
          i.file_name.toLowerCase().includes(query) ||
          (i.source?.title ?? "").toLowerCase().includes(query),
      )
    : items;
  const enabledCount = items.filter((i) => i.enabled).length;

  const addContent = async () => {
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
    await api.toggleInstanceContent(instance.id, tab, item.file_name);
    await refresh();
  };

  const askRemove = async (item: ContentItem) => {
    const dependents = await api
      .getContentDependents(instance.id, tab, item.file_name)
      .catch(() => [] as string[]);
    if (dependents.length === 0 && item.source?.origin !== "pack") {
      await remove(item);
      return;
    }
    setConfirmDelete({ item, dependents });
  };

  const remove = async (item: ContentItem) => {
    setConfirmDelete(null);
    await api.deleteInstanceContent(instance.id, tab, item.file_name);
    await refresh();
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
        await applyUpdate(instance.id, update.kind, update.file_name);
        done += 1;
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
    await applyUpdate(instance.id, tab, item.file_name);
    await refresh();
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col">
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

        <button
          onClick={() => setView("instances")}
          className="absolute left-4 top-12 grid size-9 place-items-center rounded-full border border-white/10 bg-black/50 text-white/80 backdrop-blur transition-colors hover:bg-black/70 hover:text-white"
        >
          <ArrowLeft className="size-4" />
        </button>

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
          {tabUpdates.length > 0 && (
            <button
              onClick={updateAll}
              disabled={updatingAll}
              className="inline-flex items-center gap-1.5 rounded-lg border border-warn/40 bg-warn/10 px-3 py-2 text-xs font-semibold text-warn transition-colors hover:bg-warn/20 disabled:opacity-60"
            >
              {updatingAll ? (
                <Loader2 className="size-3.5 animate-spin" />
              ) : (
                <ArrowUpCircle className="size-3.5" />
              )}
              Update all ({tabUpdates.length})
            </button>
          )}
          <button
            onClick={checkUpdates}
            disabled={checkingUpdates}
            title="Check for updates"
            aria-label="Check for updates"
            className="grid size-9 place-items-center rounded-lg border border-border bg-surface-2 text-content-faint transition-colors hover:bg-surface-3 hover:text-content disabled:opacity-60"
          >
            <RefreshCw className={cn("size-3.5", checkingUpdates && "animate-spin")} />
          </button>
          <button
            onClick={addContent}
            className="inline-flex items-center gap-1.5 rounded-lg px-3 py-2 text-xs font-semibold text-black shadow-md shadow-[var(--accent-glow)] transition-all [background:linear-gradient(to_bottom,var(--accent),var(--accent-deep))] hover:[background:linear-gradient(to_bottom,var(--accent-bright),var(--accent))]"
          >
            <Plus className="size-3.5" />
            Add content
          </button>
        </div>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto">
        <div className="px-6 py-5">
        {items.length > 0 && (
          <div className="mb-4 flex items-center gap-3">
            <div className="relative w-full max-w-sm">
              <Search className="absolute left-3 top-1/2 size-3.5 -translate-y-1/2 text-content-faint" />
              <input
                value={filter}
                onChange={(e) => setFilter(e.target.value)}
                placeholder={`Filter ${tabMeta.label.toLowerCase()}`}
                className="w-full rounded-lg border border-border bg-base py-2 pl-9 pr-3 text-sm text-content outline-none transition-colors focus:border-[var(--accent)]"
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
          <div className="py-16 text-center text-sm text-content-faint">
            Nothing matches “{filter}”.
          </div>
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
                      openProject(source!.provider!, source!.project_id!, tab)
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
                      disabled={busy}
                      title={`Update to ${item.update.latest_name}`}
                      className="inline-flex h-8 shrink-0 items-center gap-1.5 rounded-lg bg-warn/15 px-3 text-xs font-semibold text-warn transition-colors hover:bg-warn/25 disabled:opacity-60"
                    >
                      {busy ? (
                        <Loader2 className="size-3.5 animate-spin" />
                      ) : (
                        <ArrowUpCircle className="size-3.5" />
                      )}
                      Update
                    </button>
                  )}

                  <Toggle on={item.enabled} onClick={() => toggle(item)} />
                  <button
                    onClick={() => askRemove(item)}
                    aria-label="Delete file"
                    className="grid size-8 place-items-center rounded-lg text-content-faint transition-colors hover:bg-danger/15 hover:text-danger"
                  >
                    <Trash2 className="size-4" />
                  </button>
                </div>
              );
            })}
          </div>
        )}
        </div>
      </div>

      {confirmDelete && (
        <div
          className="fixed inset-0 z-[60] grid place-items-center bg-black/60 p-6 backdrop-blur-sm"
          onClick={() => setConfirmDelete(null)}
        >
          <div
            onClick={(e) => e.stopPropagation()}
            className="w-full max-w-md overflow-hidden rounded-2xl border border-border bg-surface shadow-2xl"
          >
            <div className="flex items-start gap-3 border-b border-border-soft px-5 py-4">
              <TriangleAlert className="mt-0.5 size-4 shrink-0 text-warn" />
              <div className="min-w-0">
                <h2 className="font-display text-base font-semibold text-content">
                  Remove {confirmDelete.item.source?.title ?? confirmDelete.item.file_name}?
                </h2>
                <div className="mt-1 text-xs text-content-muted">
                  {confirmDelete.dependents.length > 0 ? (
                    <>
                      {confirmDelete.dependents.length === 1
                        ? "This mod is required by "
                        : "This mod is required by "}
                      <span className="font-medium text-warn">
                        {confirmDelete.dependents.join(", ")}
                      </span>
                      . Removing it will likely break the game.
                    </>
                  ) : (
                    "This file came from a modpack. Removing it may break the pack."
                  )}
                </div>
              </div>
            </div>
            <div className="flex items-center justify-end gap-2 px-5 py-4">
              <button
                onClick={() => setConfirmDelete(null)}
                className="rounded-lg px-3 py-2 text-sm font-medium text-content-muted transition-colors hover:text-content"
              >
                Keep it
              </button>
              <button
                onClick={() => remove(confirmDelete.item)}
                className="rounded-lg bg-danger/15 px-4 py-2 text-sm font-semibold text-danger transition-colors hover:bg-danger/25"
              >
                Remove anyway
              </button>
            </div>
          </div>
        </div>
      )}

      <EditInstanceModal
        instance={editOpen ? instance : null}
        onClose={() => setEditOpen(false)}
      />
    </div>
  );
}
