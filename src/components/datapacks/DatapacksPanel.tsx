import { useCallback, useEffect, useMemo, useState } from "react";
import {
  ArrowUpCircle,
  ChevronDown,
  ChevronRight,
  Compass,
  FileUp,
  Folder,
  FolderOpen,
  Globe2,
  Loader2,
  Package,
  Plus,
  Trash2,
  TriangleAlert,
} from "lucide-react";
import { toast } from "sonner";

import { api } from "../../lib/api";
import { cn } from "../../lib/cn";
import { log } from "../../lib/log";
import { openFolder } from "../../lib/reveal";
import { formatBytes } from "../../lib/format";
import { useStore } from "../../store";
import type { Datapack, Instance, WorldPacks, WorldSummary } from "../../lib/types";
import { ConfirmDialog } from "../ConfirmDialog";
import { ContextMenu, useContextMenu } from "../ContextMenu";
import { Modal, ModalHeader } from "../Modal";
import { UploadModal } from "../UploadModal";
import { EmptyState } from "../ui";

function formatRange(pack: Datapack) {
  if (pack.min_format == null) return "format unknown";
  if (pack.max_format == null || pack.max_format === pack.min_format) {
    return `format ${pack.min_format}`;
  }
  return `format ${pack.min_format} to ${pack.max_format}`;
}

function Shimmer({ className }: { className?: string }) {
  return <div className={cn("animate-pulse rounded bg-surface-3/50", className)} />;
}

function DatapacksSkeleton() {
  return (
    <div className="px-6 py-4" aria-busy="true" aria-label="Reading the worlds">
      {["w-40", "w-32"].map((width) => (
        <div key={width} className="mb-6">
          <div className="flex items-center gap-2.5 py-2">
            <Shimmer className="size-3.5" />
            <Shimmer className={cn("h-4", width)} />
          </div>
          {["w-56", "w-64", "w-44"].map((row) => (
            <div key={row} className="flex items-center gap-3 py-2.5 pl-6">
              <Shimmer className="size-4 rounded-full" />
              <div className="min-w-0 flex-1">
                <Shimmer className={cn("h-3.5", row)} />
                <Shimmer className="mt-1.5 h-3 w-28" />
              </div>
              <Shimmer className="h-3 w-12" />
            </div>
          ))}
        </div>
      ))}
    </div>
  );
}

function State({ pack }: { pack: Datapack }) {
  if (!pack.enabled) {
    return (
      <span className="shrink-0 rounded border border-border bg-surface-3 px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-content-faint">
        Off
      </span>
    );
  }
  if (pack.off_in_game) {
    return (
      <span
        title="The file is here, but this world has it switched off"
        className="shrink-0 rounded border border-warn/30 bg-warn/10 px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-warn"
      >
        Off in game
      </span>
    );
  }
  return (
    <span className="shrink-0 rounded border border-ok/30 bg-ok/10 px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-ok">
      Active
    </span>
  );
}

function PackRow({
  pack,
  gameVersion,
  busy,
  onToggle,
  onDelete,
  onUpdate,
}: {
  pack: Datapack;
  gameVersion: string;
  busy: boolean;
  onToggle: () => void;
  onDelete: () => void;
  onUpdate: () => void;
}) {
  const mismatch = pack.compatibility.state === "mismatch" ? pack.compatibility : null;

  return (
    <div className="group/row flex items-center gap-3 border-b border-border-soft/60 py-2.5 pl-6 last:border-b-0">
      <span className="grid size-8 shrink-0 place-items-center rounded-lg border border-border-soft bg-surface-2 text-content-faint">
        {pack.directory ? <Folder className="size-4" /> : <Package className="size-4" />}
      </span>

      <div className="min-w-0 flex-1">
        <div className="flex items-baseline gap-2">
          <span
            className={cn(
              "truncate text-[13px] font-medium",
              pack.enabled ? "text-content" : "text-content-faint",
            )}
          >
            {pack.title ?? pack.file_name}
          </span>
          <State pack={pack} />
        </div>
        <div className="mt-0.5 flex flex-wrap items-center gap-x-2 gap-y-0.5 text-[11px] text-content-faint">
          <span className="truncate font-mono">{pack.file_name}</span>
          <span>·</span>
          <span>{formatRange(pack)}</span>
          <span>·</span>
          <span>{formatBytes(pack.size)}</span>
          {mismatch && (
            <>
              <span>·</span>
              <span
                title="Minecraft marks a datapack outside its format range as incompatible"
                className="inline-flex items-center gap-1 text-warn"
              >
                <TriangleAlert className="size-3" />
                Minecraft {gameVersion} wants format {mismatch.needs}
              </span>
            </>
          )}
        </div>
      </div>

      {pack.latest_version_id && (
        <button
          onClick={onUpdate}
          disabled={busy}
          className="inline-flex shrink-0 items-center gap-1.5 rounded-lg border border-warn/40 bg-warn/10 px-2.5 py-1 text-[11px] font-semibold text-warn transition-colors hover:bg-warn/20 disabled:opacity-50"
        >
          <ArrowUpCircle className="size-3.5" />
          Update
        </button>
      )}

      <button
        onClick={onToggle}
        disabled={busy}
        className="shrink-0 rounded-lg border border-border bg-surface-2 px-2.5 py-1 text-[11px] font-medium text-content-muted opacity-0 transition-colors hover:bg-surface-3 hover:text-content focus-visible:opacity-100 disabled:opacity-50 group-hover/row:opacity-100"
      >
        {pack.enabled ? "Turn off" : "Turn on"}
      </button>
      <button
        onClick={onDelete}
        disabled={busy}
        aria-label={`Delete ${pack.file_name}`}
        className="grid size-7 shrink-0 place-items-center rounded-lg text-content-faint opacity-0 transition-colors hover:bg-danger/10 hover:text-danger focus-visible:opacity-100 disabled:opacity-50 group-hover/row:opacity-100"
      >
        <Trash2 className="size-3.5" />
      </button>
    </div>
  );
}

export function DatapacksPanel({
  instance,
  refreshToken,
  addFor,
  onAddHandled,
}: {
  instance: Instance;
  refreshToken: number;
  addFor: number;
  onAddHandled: () => void;
}) {
  const openDiscover = useStore((s) => s.openDiscover);
  const { menu, open: openMenu, close: closeMenu } = useContextMenu();
  const [groups, setGroups] = useState<WorldPacks[]>([]);
  const [loading, setLoading] = useState(true);
  const [closed, setClosed] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const [adding, setAdding] = useState<WorldPacks | null>(null);
  const [removing, setRemoving] = useState<{ world: string; pack: Datapack } | null>(null);
  const [worlds, setWorlds] = useState<WorldSummary[]>([]);
  const [choosingWorld, setChoosingWorld] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const [listed, allWorlds] = await Promise.all([
        api.listInstanceDatapacks(instance.id),
        api.listInstanceWorlds(instance.id).catch(() => [] as WorldSummary[]),
      ]);
      setWorlds(allWorlds);
      setGroups(listed);
    } catch (cause) {
      log.warn("datapacks", `could not list datapacks: ${String(cause)}`);
      setGroups([]);
    } finally {
      setLoading(false);
    }
  }, [instance.id]);

  useEffect(() => {
    setLoading(true);
    void refresh();
  }, [refresh, refreshToken]);

  useEffect(() => {
    if (addFor > 0) setChoosingWorld(true);
  }, [addFor]);

  useEffect(() => {
    void api.checkDatapackUpdates(instance.id).then((found) => {
      if (found > 0) void refresh();
    });
  }, [instance.id, refresh]);

  const total = useMemo(
    () => groups.reduce((sum, group) => sum + group.packs.length, 0),
    [groups],
  );

  const addMenu = (event: React.MouseEvent, group: WorldPacks) => {
    openMenu(
      event,
      [
        {
          label: "Browse datapacks",
          icon: Compass,
          onSelect: () => openDiscover("datapacks", instance.id, group.world || null),
          disabled: group.loose,
        },
        {
          label: "Add files from disk",
          icon: FileUp,
          onSelect: () => setAdding(group),
        },
      ],
      group.display_name,
      { below: true },
    );
  };

  const run = async (work: () => Promise<unknown>) => {
    setBusy(true);
    try {
      await work();
      await refresh();
    } catch (cause) {
      toast.error("That did not work", { description: String(cause) });
    } finally {
      setBusy(false);
    }
  };

  if (loading) return <DatapacksSkeleton />;

  if (groups.length === 0) {
    return (
      <EmptyState
        icon={<Package className="size-6" />}
        title="No datapacks yet"
        description="Datapacks belong to a single world. Open a world here once it has one, or browse for a pack and pick where it goes."
      />
    );
  }

  return (
    <div className="px-6 py-4">
      <p className="mb-3 text-xs text-content-muted">
        {total} {total === 1 ? "datapack" : "datapacks"} across{" "}
        {groups.length === 1 ? "one world" : `${groups.length} worlds`}
      </p>

      {groups.map((group) => {
        const open = !closed.includes(group.world);
        return (
          <div key={group.world || "loose"} className="mb-4">
            <div className="flex items-center gap-2.5">
              <button
                onClick={() =>
                  setClosed((current) =>
                    open
                      ? [...current, group.world]
                      : current.filter((value) => value !== group.world),
                  )
                }
                className="flex min-w-0 flex-1 items-center gap-2.5 py-2 text-left"
              >
                <ChevronRight
                  className={cn(
                    "size-3.5 shrink-0 text-content-faint transition-transform",
                    open && "rotate-90",
                  )}
                />
                <Globe2 className="size-4 shrink-0 text-content-faint" />
                <span className="truncate text-sm font-medium text-content">
                  {group.display_name}
                </span>
                <span className="shrink-0 text-[11px] text-content-faint">
                  {group.packs.length}
                </span>
              </button>

              <button
                onClick={() =>
                  openFolder(
                    group.loose
                      ? `${instance.dir}/datapacks`
                      : `${instance.dir}/saves/${group.world}/datapacks`,
                  )
                }
                aria-label="Open the folder"
                className="grid size-7 shrink-0 place-items-center rounded-lg text-content-faint transition-colors hover:bg-surface-3 hover:text-content"
              >
                <FolderOpen className="size-3.5" />
              </button>
              <button
                onClick={(event) => addMenu(event, group)}
                className="inline-flex shrink-0 items-center gap-1.5 rounded-lg border border-border bg-surface-2 px-2.5 py-1 text-[11px] font-medium text-content-muted transition-colors hover:bg-surface-3 hover:text-content"
              >
                <Plus className="size-3.5" />
                Add
                <ChevronDown className="size-3 opacity-70" />
              </button>
            </div>

            {group.loose && open && (
              <p className="mb-1 pl-6 text-[11px] text-warn">
                These came in with a modpack and sit outside every world, so Minecraft never
                loads them. Move them into a world folder to use them.
              </p>
            )}

            {open && (
              <div>
                {group.packs.map((pack) => (
                  <PackRow
                    key={pack.file_name}
                    pack={pack}
                    gameVersion={instance.version_id}
                    busy={busy}
                    onToggle={() =>
                      void run(() =>
                        api.toggleDatapack(instance.id, group.world, pack.file_name),
                      )
                    }
                    onDelete={() => setRemoving({ world: group.world, pack })}
                    onUpdate={() =>
                      void run(() =>
                        api
                          .applyDatapackUpdate(instance.id, group.world, pack.file_name)
                          .then(() => toast.success(`Updated ${pack.title ?? pack.file_name}`)),
                      )
                    }
                  />
                ))}
              </div>
            )}
          </div>
        );
      })}

      {adding && (
        <UploadModal
          open
          onClose={() => setAdding(null)}
          title={`Add datapacks to ${adding.display_name}`}
          subtitle="Zip packs, copied straight into that world"
          extensions={["zip"]}
          filterName="Datapack"
          multiple
          busy={busy}
          onConfirm={(paths) => {
            const world = adding.world;
            setAdding(null);
            void run(() => api.addDatapacks(instance.id, world, paths));
          }}
        />
      )}

      <ConfirmDialog
        open={!!removing}
        title={`Delete ${removing?.pack.title ?? removing?.pack.file_name}?`}
        description="The pack is removed from that world's folder. This cannot be undone."
        confirmIcon={<Trash2 className="size-3.5" />}
        onConfirm={() => {
          const target = removing;
          setRemoving(null);
          if (target) {
            void run(() =>
              api.deleteDatapack(instance.id, target.world, target.pack.file_name),
            );
          }
        }}
        onCancel={() => setRemoving(null)}
      />

      {choosingWorld && (
        <Modal
          open
          onClose={() => {
            setChoosingWorld(false);
            onAddHandled();
          }}
          size="md"
          className="h-[min(520px,calc(100vh-48px))]"
          labelledBy="pick-world-title"
        >
          <ModalHeader
            id="pick-world-title"
            title="Which world?"
            subtitle="A datapack belongs to one world, not the whole instance"
            onClose={() => {
              setChoosingWorld(false);
              onAddHandled();
            }}
          />
          <div className="flex min-h-0 flex-1 flex-col gap-2 overflow-y-auto px-5 py-4">
            {worlds.length === 0 && (
              <p className="py-6 text-sm text-content-faint">
                This instance has no worlds yet.
              </p>
            )}
            {worlds.map((world) => (
              <button
                key={world.folder_name}
                onClick={() => {
                  setChoosingWorld(false);
                  onAddHandled();
                  setAdding({
                    world: world.folder_name,
                    display_name: world.name,
                    loose: false,
                    packs: [],
                  });
                }}
                className="flex items-center gap-3 rounded-xl border border-border-soft bg-surface-2/50 px-3.5 py-3 text-left transition-colors hover:border-border hover:bg-surface-2"
              >
                <Globe2 className="size-4 shrink-0 text-content-faint" />
                <span className="min-w-0 flex-1">
                  <span className="block truncate text-sm font-medium text-content">
                    {world.name}
                  </span>
                  <span className="block truncate font-mono text-[11px] text-content-faint">
                    {world.folder_name}
                  </span>
                </span>
              </button>
            ))}
          </div>
        </Modal>
      )}

      <ContextMenu menu={menu} onClose={closeMenu} />

      {busy && (
        <div className="pointer-events-none fixed bottom-6 right-6 inline-flex items-center gap-2 rounded-lg border border-border bg-surface-2 px-3 py-2 text-xs text-content-muted shadow-lg">
          <Loader2 className="size-3.5 animate-spin" />
          Working
        </div>
      )}
    </div>
  );
}
