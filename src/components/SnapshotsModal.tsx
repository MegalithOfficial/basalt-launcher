import { useEffect, useMemo, useState } from "react";
import {
  ArchiveRestore,
  Check,
  DatabaseBackup,
  HardDrive,
  Loader2,
  Pencil,
  RotateCcw,
  ShieldCheck,
  Trash2,
  X,
} from "lucide-react";
import { toast } from "sonner";

import { api } from "../lib/api";
import { cn } from "../lib/cn";
import { relativeTime } from "../lib/time";
import { taskFraction, useInstanceTask } from "../lib/useTasks";
import type { Instance, SnapshotSummary } from "../lib/types";
import { ConfirmDialog } from "./ConfirmDialog";
import { Modal, ModalBody, ModalHeader } from "./Modal";
import { formatBytes } from "../lib/format";


function SnapshotRow({
  snapshot,
  editing,
  editingName,
  disabled,
  onEditingName,
  onStartEdit,
  onSubmitEdit,
  onCancelEdit,
  onRestore,
  onDelete,
}: {
  snapshot: SnapshotSummary;
  editing: boolean;
  editingName: string;
  disabled: boolean;
  onEditingName: (value: string) => void;
  onStartEdit: () => void;
  onSubmitEdit: () => void;
  onCancelEdit: () => void;
  onRestore: () => void;
  onDelete: () => void;
}) {
  const automatic = snapshot.kind === "automatic";
  return (
    <div className="group flex items-center gap-3 rounded-xl border border-border-soft bg-surface-2/50 px-3.5 py-3 transition-colors hover:border-border hover:bg-surface-2">
      <span
        className={cn(
          "grid size-9 shrink-0 place-items-center rounded-lg",
          automatic ? "bg-surface-3 text-content-faint" : "bg-(--accent)/10 text-(--accent)",
        )}
      >
        {automatic ? <ShieldCheck className="size-4" /> : <DatabaseBackup className="size-4" />}
      </span>

      <div className="min-w-0 flex-1">
        {editing ? (
          <form
            onSubmit={(event) => {
              event.preventDefault();
              onSubmitEdit();
            }}
            className="flex max-w-md items-center gap-1.5"
          >
            <input
              autoFocus
              value={editingName}
              onChange={(event) => onEditingName(event.target.value)}
              maxLength={80}
              className="h-7 min-w-0 flex-1 rounded-md border border-(--accent)/40 bg-base px-2 text-sm font-medium text-content outline-none"
            />
            <button
              type="submit"
              aria-label="Save name"
              className="grid size-7 place-items-center text-ok"
            >
              <Check className="size-3.5" />
            </button>
            <button
              type="button"
              onClick={onCancelEdit}
              aria-label="Cancel"
              className="grid size-7 place-items-center text-content-faint hover:text-content"
            >
              <X className="size-3.5" />
            </button>
          </form>
        ) : (
          <div className="truncate text-sm font-medium text-content">{snapshot.name}</div>
        )}
        <div className="mt-0.5 truncate text-[11px] text-content-faint">
          {relativeTime(snapshot.created_at)} ·{" "}
          {snapshot.new_size_bytes == null
            ? "Storage contribution unavailable"
            : snapshot.new_size_bytes > 0
            ? `Added ${formatBytes(snapshot.new_size_bytes)}`
            : "No additional storage"} ·{" "}
          {snapshot.file_count} {snapshot.file_count === 1 ? "file" : "files"}
        </div>
      </div>

      <div className="flex shrink-0 items-center gap-1 opacity-0 transition-opacity focus-within:opacity-100 group-hover:opacity-100">
        <button
          onClick={onStartEdit}
          aria-label="Rename"
          title="Rename"
          className="grid size-8 place-items-center rounded-lg text-content-faint transition-colors hover:bg-surface-3 hover:text-content"
        >
          <Pencil className="size-3.5" />
        </button>
        <button
          onClick={onDelete}
          disabled={disabled}
          aria-label="Delete"
          title="Delete"
          className="grid size-8 place-items-center rounded-lg text-content-faint transition-colors hover:bg-danger/15 hover:text-danger disabled:opacity-30"
        >
          <Trash2 className="size-3.5" />
        </button>
      </div>

      <button
        onClick={onRestore}
        disabled={disabled}
        className="inline-flex h-8 shrink-0 items-center gap-1.5 rounded-lg border border-border bg-surface-2 px-3 text-xs font-medium text-content-muted transition-colors hover:bg-surface-3 hover:text-content disabled:cursor-not-allowed disabled:opacity-30"
      >
        <RotateCcw className="size-3.5" />
        Restore
      </button>
    </div>
  );
}

export function SnapshotsModal({
  instance,
  open,
  running,
  busyWithTask,
  onClose,
  onRestored,
}: {
  instance: Instance;
  open: boolean;
  running: boolean;
  busyWithTask: boolean;
  onClose: () => void;
  onRestored: () => Promise<void>;
}) {
  const [snapshots, setSnapshots] = useState<SnapshotSummary[]>([]);
  const [loading, setLoading] = useState(false);
  const [creating, setCreating] = useState(false);
  const [restoringNow, setRestoringNow] = useState(false);
  const [name, setName] = useState("");
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editingName, setEditingName] = useState("");
  const [restoring, setRestoring] = useState<SnapshotSummary | null>(null);
  const [removing, setRemoving] = useState<SnapshotSummary | null>(null);
  const [freeMb, setFreeMb] = useState<number | null>(null);

  const task = useInstanceTask(instance.id);
  const snapshotTask =
    task && (task.kind === "snapshot_create" || task.kind === "snapshot_restore") ? task : null;
  const working = creating || restoringNow;
  const blocked = running || busyWithTask;
  const unavailable = blocked || working;

  const manual = useMemo(
    () => snapshots.filter((snapshot) => snapshot.kind !== "automatic"),
    [snapshots],
  );
  const automatic = useMemo(
    () => snapshots.filter((snapshot) => snapshot.kind === "automatic"),
    [snapshots],
  );
  const [usedBytes, setUsedBytes] = useState<number | null>(null);

  const refreshUsage = () => {
    api
      .instanceSnapshotUsage(instance.id)
      .then(setUsedBytes)
      .catch(() => setUsedBytes(null));
  };

  const load = async () => {
    setLoading(true);
    try {
      setSnapshots(await api.listInstanceSnapshots(instance.id));
      refreshUsage();
    } catch (error) {
      toast.error("Could not load snapshots", { description: String(error) });
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    if (!open) return;
    void load();
    api
      .getSystemStats()
      .then((stats) => setFreeMb(stats.data_dir_free_mb))
      .catch(() => setFreeMb(null));
  }, [open, instance.id]);

  const create = async () => {
    if (unavailable) return;
    setCreating(true);
    try {
      const snapshot = await api.createInstanceSnapshot(instance.id, name.trim() || null);
      setSnapshots((current) => [snapshot, ...current]);
      setName("");
      refreshUsage();
      toast.success("Snapshot created", {
        description:
          snapshot.new_size_bytes != null && snapshot.new_size_bytes > 0
            ? `Added ${formatBytes(snapshot.new_size_bytes)} for ${snapshot.file_count} ${snapshot.file_count === 1 ? "file" : "files"}.`
            : `Reused existing data for all ${snapshot.file_count} ${snapshot.file_count === 1 ? "file" : "files"}.`,
      });
    } catch (error) {
      if (!/cancelled/i.test(String(error))) {
        toast.error("Could not create snapshot", { description: String(error) });
      }
    } finally {
      setCreating(false);
    }
  };

  const rename = async (snapshot: SnapshotSummary) => {
    const next = editingName.trim();
    if (!next) return;
    try {
      const updated = await api.renameInstanceSnapshot(instance.id, snapshot.id, next);
      setSnapshots((current) => current.map((item) => (item.id === updated.id ? updated : item)));
      setEditingId(null);
    } catch (error) {
      toast.error("Could not rename snapshot", { description: String(error) });
    }
  };

  const rowProps = (snapshot: SnapshotSummary) => ({
    snapshot,
    editing: editingId === snapshot.id,
    editingName,
    disabled: unavailable,
    onEditingName: setEditingName,
    onStartEdit: () => {
      setEditingId(snapshot.id);
      setEditingName(snapshot.name);
    },
    onSubmitEdit: () => void rename(snapshot),
    onCancelEdit: () => setEditingId(null),
    onRestore: () => setRestoring(snapshot),
    onDelete: () => setRemoving(snapshot),
  });

  return (
    <>
      <Modal
        open={open}
        onClose={onClose}
        size="wide"
        labelledBy="snapshots-title"
        dismissable={!working}
        className="h-[min(660px,calc(100vh-48px))]"
      >
        <ModalHeader
          id="snapshots-title"
          title={
            <span className="flex items-center gap-2">
              Snapshots
              <span
                title="Snapshots are still under development. Creating and restoring them has been tested, but not on every setup or every edge case. Keep your own copy of anything you cannot afford to lose."
                className="cursor-help rounded-md border border-warn/30 bg-warn/10 px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-warn"
              >
                Experimental
              </span>
            </span>
          }
          subtitle={`Restore points for ${instance.name}`}
          icon={
            <div className="grid size-9 place-items-center rounded-xl border border-border-soft bg-surface-2 text-(--accent)">
              <ArchiveRestore className="size-4" />
            </div>
          }
          onClose={working ? undefined : onClose}
        />

        <div className="shrink-0 border-b border-border-soft px-5 py-4">
          <div className="flex items-center gap-2">
            <input
              value={name}
              onChange={(event) => setName(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter") void create();
              }}
              disabled={unavailable}
              maxLength={80}
              placeholder="Name this restore point (optional)"
              className="h-9 min-w-0 flex-1 rounded-lg border border-border bg-base px-3 text-sm text-content outline-none transition-colors placeholder:text-content-faint focus:border-(--accent) disabled:opacity-40"
            />
            <button
              onClick={() => void create()}
              disabled={unavailable}
              className="inline-flex h-9 shrink-0 items-center gap-2 rounded-lg px-4 text-xs font-semibold text-black shadow-md shadow-(color:--accent-glow) transition-all [background:linear-gradient(to_bottom,var(--accent),var(--accent-deep))] hover:[background:linear-gradient(to_bottom,var(--accent-bright),var(--accent))] disabled:cursor-not-allowed disabled:opacity-40 disabled:shadow-none"
            >
              {creating ? (
                <Loader2 className="size-3.5 animate-spin" />
              ) : (
                <DatabaseBackup className="size-3.5" />
              )}
              Take snapshot
            </button>
          </div>
          <p className="mt-2 text-[11px] leading-relaxed text-content-faint">
            Saves worlds, mods, configs, options and launcher settings. Logs, crash reports and
            screenshots are skipped.
          </p>

          {snapshotTask ? (
            <div className="mt-3 rounded-lg border border-(--accent)/25 bg-(--accent)/[0.07] px-3 py-2.5">
              <div className="flex items-baseline justify-between gap-3 text-[11px]">
                <span className="font-medium text-content">
                  {snapshotTask.kind === "snapshot_create"
                    ? "Indexing and compressing files"
                    : "Verifying and restoring files"}
                </span>
                <span className="tabular-nums text-content-faint">
                  {snapshotTask.total > 0
                    ? `${snapshotTask.completed} of ${snapshotTask.total} files`
                    : snapshotTask.stage}
                </span>
              </div>
              <div className="mt-2 h-1 overflow-hidden rounded-full bg-surface-3">
                <div
                  className={cn(
                    "h-full rounded-full bg-(--accent)",
                    taskFraction(snapshotTask) == null
                      ? "w-1/3 animate-pulse"
                      : "transition-[width] duration-300",
                  )}
                  style={
                    taskFraction(snapshotTask) == null
                      ? undefined
                      : { width: `${Math.round((taskFraction(snapshotTask) ?? 0) * 100)}%` }
                  }
                />
              </div>
            </div>
          ) : (
            blocked && (
              <div className="mt-3 rounded-lg border border-warn/25 bg-warn/10 px-3 py-2 text-[11px] text-warn">
                {running
                  ? "Stop the game before taking or restoring a snapshot."
                  : "Wait for the current instance operation to finish."}
              </div>
            )
          )}
        </div>

        <ModalBody className="flex flex-col gap-3">
          <div className="flex items-center gap-2 text-[11px] text-content-faint">
            {snapshots.length > 0 && (
              <>
                <span>
                  {snapshots.length}{" "}
                  {snapshots.length === 1 ? "restore point" : "restore points"}
                </span>
                {usedBytes != null && (
                  <>
                    <span className="size-1 rounded-full bg-border" />
                    <span>{formatBytes(usedBytes)} on disk</span>
                  </>
                )}
              </>
            )}
            {freeMb != null && (
              <span className="ml-auto inline-flex items-center gap-1.5">
                <HardDrive className="size-3.5" />
                {(freeMb / 1024).toFixed(1)} GB free
              </span>
            )}
          </div>

          {loading ? (
            <div className="flex flex-1 items-center justify-center gap-2 text-sm text-content-muted">
              <Loader2 className="size-4 animate-spin" />
              Reading snapshots
            </div>
          ) : snapshots.length === 0 ? (
            <div className="flex flex-1 flex-col items-center justify-center gap-3 text-center">
              <div className="grid size-12 place-items-center rounded-2xl border border-border-soft bg-surface-2 text-content-faint">
                <DatabaseBackup className="size-6" />
              </div>
              <div className="text-sm font-medium text-content-muted">No restore points yet</div>
              <p className="max-w-xs text-xs leading-relaxed text-content-faint">
                Take one before changing loaders, updating a pack, or trying mods you are unsure
                about.
              </p>
            </div>
          ) : (
            <div className="flex flex-col gap-2">
              {manual.map((snapshot) => (
                <SnapshotRow key={snapshot.id} {...rowProps(snapshot)} />
              ))}

              {automatic.length > 0 && (
                <>
                  <div className="mt-3 flex items-center gap-2.5">
                    <span className="text-[11px] font-medium text-content-muted">
                      Safety copies
                    </span>
                    <span className="h-px flex-1 bg-border-soft" />
                    <span className="text-[11px] text-content-faint">
                      taken before each restore, newest three kept
                    </span>
                  </div>
                  {automatic.map((snapshot) => (
                    <SnapshotRow key={snapshot.id} {...rowProps(snapshot)} />
                  ))}
                </>
              )}
            </div>
          )}
        </ModalBody>
      </Modal>

      <ConfirmDialog
        open={!!restoring}
        nested
        tone="warn"
        title={restoring ? `Restore ${restoring.name}?` : "Restore snapshot?"}
        description="Every file in this instance goes back to how it was. Basalt saves the current state as a safety copy first, so you can undo it."
        confirmLabel="Restore"
        confirmIcon={<ArchiveRestore className="size-3.5" />}
        onConfirm={() => {
          const target = restoring;
          if (!target) return;
          setRestoring(null);
          setRestoringNow(true);
          void (async () => {
            try {
              const restored = await api.restoreInstanceSnapshot(instance.id, target.id);
              await onRestored();
              await load();
              toast.success(`Restored ${restored.name}`, {
                description: "The previous state was saved as a safety copy.",
              });
            } catch (error) {
              if (!/cancelled/i.test(String(error))) {
                toast.error(`Could not restore ${target.name}`, {
                  description: String(error),
                });
              }
            } finally {
              setRestoringNow(false);
            }
          })();
        }}
        onCancel={() => setRestoring(null)}
      />

      <ConfirmDialog
        open={!!removing}
        nested
        title={removing ? `Delete ${removing.name}?` : "Delete snapshot?"}
        description="This restore point is removed. Data still used by another snapshot is kept, and unreferenced data is cleaned up. The instance itself is untouched."
        confirmLabel="Delete snapshot"
        onConfirm={async () => {
          if (!removing) return;
          await api.deleteInstanceSnapshot(instance.id, removing.id);
          setSnapshots((current) => current.filter((item) => item.id !== removing.id));
          setRemoving(null);
          refreshUsage();
        }}
        onCancel={() => setRemoving(null)}
      />
    </>
  );
}
