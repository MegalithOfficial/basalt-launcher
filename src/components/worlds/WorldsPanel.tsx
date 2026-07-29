import { useCallback, useEffect, useMemo, useState } from "react";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { openPath } from "@tauri-apps/plugin-opener";
import {
  Archive,
  Check,
  FolderOpen,
  Globe2,
  HardDriveUpload,
  Loader2,
  Search,
  ShieldAlert,
  Trash2,
  TriangleAlert,
} from "lucide-react";

import { api } from "../../lib/api";
import { cn } from "../../lib/cn";
import { relativeTime } from "../../lib/time";
import type {
  Instance,
  WorldImportCandidate,
  WorldImportInspection,
  WorldStatus,
  WorldSummary,
} from "../../lib/types";
import { Modal, ModalHeader } from "../Modal";

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 ** 2) return `${(bytes / 1024).toFixed(0)} KB`;
  if (bytes < 1024 ** 3) return `${(bytes / 1024 ** 2).toFixed(1)} MB`;
  return `${(bytes / 1024 ** 3).toFixed(1)} GB`;
}

function versionLabel(versionName: string | null, dataVersion: number | null): string {
  if (versionName) return versionName;
  if (dataVersion != null) return `Data version ${dataVersion}`;
  return "Legacy version";
}

function difficultyLabel(difficulty: number | null): string | null {
  switch (difficulty) {
    case 0:
      return "Peaceful";
    case 1:
      return "Easy";
    case 2:
      return "Normal";
    case 3:
      return "Hard";
    default:
      return null;
  }
}

function statusLabel(status: WorldStatus): string | null {
  if (status === "recovered") return "Recovered metadata";
  if (status === "damaged") return "Damaged metadata";
  return null;
}

function StatusBadge({ status }: { status: WorldStatus }) {
  const label = statusLabel(status);
  if (!label) return null;
  return (
    <span
      className={cn(
        "shrink-0 rounded-md px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-wide",
        status === "damaged"
          ? "bg-danger/15 text-danger"
          : "bg-warn/15 text-warn",
      )}
    >
      {label}
    </span>
  );
}

function CandidateRow({
  candidate,
  selected,
  onToggle,
}: {
  candidate: WorldImportCandidate;
  selected: boolean;
  onToggle: () => void;
}) {
  const disabled = candidate.status === "damaged";
  return (
    <button
      type="button"
      disabled={disabled}
      onClick={onToggle}
      className={cn(
        "flex w-full items-start gap-3 rounded-xl border px-3.5 py-3 text-left transition-colors",
        selected
          ? "border-[var(--accent)]/60 bg-[var(--accent-glow)]"
          : "border-border-soft bg-base/60 hover:border-border",
        disabled && "cursor-not-allowed opacity-55",
      )}
    >
      <span
        className={cn(
          "mt-0.5 grid size-4 shrink-0 place-items-center rounded border",
          selected
            ? "border-[var(--accent)] bg-[var(--accent)] text-black"
            : "border-border bg-surface-3",
        )}
      >
        {selected && <Check className="size-3" strokeWidth={3} />}
      </span>
      <span className="min-w-0 flex-1">
        <span className="flex items-center gap-2">
          <span className="truncate text-sm font-medium text-content">
            {candidate.name}
          </span>
          <StatusBadge status={candidate.status} />
        </span>
        <span className="mt-0.5 block truncate text-[11px] text-content-faint">
          {candidate.archive_root || "Selected folder"} ·{" "}
          {versionLabel(candidate.version_name, candidate.data_version)} ·{" "}
          {candidate.file_count.toLocaleString()} files · {formatSize(candidate.total_bytes)}
        </span>
        {candidate.error && (
          <span className="mt-1 block text-[11px] text-danger">{candidate.error}</span>
        )}
      </span>
    </button>
  );
}

function ImportDialog({
  instance,
  running,
  onClose,
  onImported,
}: {
  instance: Instance;
  running: boolean;
  onClose: () => void;
  onImported: () => Promise<void>;
}) {
  const [sourcePath, setSourcePath] = useState<string | null>(null);
  const [inspection, setInspection] = useState<WorldImportInspection | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [inspecting, setInspecting] = useState(false);
  const [importing, setImporting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const choose = async (kind: "directory" | "zip") => {
    const picked = await openFileDialog({
      multiple: false,
      directory: kind === "directory",
      ...(kind === "zip"
        ? { filters: [{ name: "World backup", extensions: ["zip"] }] }
        : {}),
    });
    if (!picked || Array.isArray(picked)) return;

    setError(null);
    setInspecting(true);
    setSourcePath(picked);
    try {
      const result = await api.inspectWorldSource(picked);
      setInspection(result);
      setSelected(
        new Set(
          result.candidates
            .filter((candidate) => candidate.status !== "damaged")
            .map((candidate) => candidate.id),
        ),
      );
    } catch (cause) {
      setInspection(null);
      setSelected(new Set());
      setError(String(cause));
    } finally {
      setInspecting(false);
    }
  };

  const toggle = (id: string) => {
    setSelected((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const submit = async () => {
    if (!sourcePath || selected.size === 0 || running) return;
    setError(null);
    setImporting(true);
    try {
      await api.importWorlds(instance.id, sourcePath, [...selected]);
      await onImported();
      onClose();
    } catch (cause) {
      setError(String(cause));
    } finally {
      setImporting(false);
    }
  };

  const busy = inspecting || importing;

  return (
    <Modal
      open
      onClose={onClose}
      size="lg"
      nested
      dismissable={!busy}
      labelledBy="world-import-title"
    >
      <ModalHeader
        id="world-import-title"
        title="Import worlds"
        subtitle={`Basalt copies worlds directly into ${instance.name}. Your files never pass through the webview.`}
        icon={
          <div className="grid size-9 place-items-center rounded-xl border border-border-soft bg-surface-2 text-[var(--accent)]">
            <HardDriveUpload className="size-4" />
          </div>
        }
        onClose={busy ? undefined : onClose}
      />

        <div className="min-h-0 flex-1 overflow-y-auto px-5 py-5">
          {!inspection && !inspecting && (
            <div className="grid gap-3 sm:grid-cols-2">
              <button
                onClick={() => choose("directory")}
                className="group rounded-xl border border-border-soft bg-base/50 p-4 text-left transition-colors hover:border-[var(--accent)]/50 hover:bg-surface-2"
              >
                <FolderOpen className="size-5 text-[var(--accent)]" />
                <div className="mt-5 text-sm font-semibold text-content">
                  World folder
                </div>
                <p className="mt-1 text-xs leading-relaxed text-content-faint">
                  Choose an extracted save containing level.dat.
                </p>
              </button>
              <button
                onClick={() => choose("zip")}
                className="group rounded-xl border border-border-soft bg-base/50 p-4 text-left transition-colors hover:border-[var(--accent)]/50 hover:bg-surface-2"
              >
                <Archive className="size-5 text-[var(--accent)]" />
                <div className="mt-5 text-sm font-semibold text-content">
                  ZIP backup
                </div>
                <p className="mt-1 text-xs leading-relaxed text-content-faint">
                  Nested folders and backups with multiple worlds are supported.
                </p>
              </button>
            </div>
          )}

          {inspecting && (
            <div className="flex flex-col items-center gap-3 py-14 text-center">
              <Loader2 className="size-5 animate-spin text-[var(--accent)]" />
              <div>
                <div className="text-sm font-medium text-content">
                  Inspecting the backup
                </div>
                <div className="mt-1 text-xs text-content-faint">
                  Checking paths, limits, and world metadata
                </div>
              </div>
            </div>
          )}

          {inspection && (
            <>
              <div className="mb-3 flex items-center justify-between gap-4">
                <div>
                  <div className="text-sm font-medium text-content">
                    {inspection.candidates.length === 1
                      ? "1 world found"
                      : `${inspection.candidates.length} worlds found`}
                  </div>
                  <div className="text-[11px] text-content-faint">
                    Select the worlds to copy. Existing saves will not be overwritten.
                  </div>
                </div>
                <button
                  onClick={() => {
                    setInspection(null);
                    setSourcePath(null);
                    setSelected(new Set());
                    setError(null);
                  }}
                  className="shrink-0 text-xs font-medium text-content-muted transition-colors hover:text-content"
                >
                  Choose another
                </button>
              </div>
              <div className="flex flex-col gap-2">
                {inspection.candidates.map((candidate) => (
                  <CandidateRow
                    key={candidate.id}
                    candidate={candidate}
                    selected={selected.has(candidate.id)}
                    onToggle={() => toggle(candidate.id)}
                  />
                ))}
              </div>
            </>
          )}

          {running && (
            <div className="mt-4 flex gap-2.5 rounded-xl border border-warn/25 bg-warn/[0.07] px-3.5 py-3 text-xs text-warn">
              <ShieldAlert className="mt-0.5 size-4 shrink-0" />
              Close the game before importing a world into this instance.
            </div>
          )}
          {error && (
            <div className="mt-4 flex gap-2.5 rounded-xl border border-danger/25 bg-danger/[0.07] px-3.5 py-3 text-xs text-danger">
              <TriangleAlert className="mt-0.5 size-4 shrink-0" />
              <span className="break-words">{error}</span>
            </div>
          )}
        </div>

        {inspection && (
          <div className="flex items-center justify-between gap-4 border-t border-border-soft px-5 py-4">
            <span className="text-xs text-content-faint">
              {selected.size} selected
            </span>
            <div className="flex items-center gap-2">
              <button
                onClick={onClose}
                disabled={importing}
                className="rounded-lg px-3 py-2 text-sm font-medium text-content-muted transition-colors hover:text-content disabled:opacity-50"
              >
                Cancel
              </button>
              <button
                onClick={submit}
                disabled={selected.size === 0 || running || importing}
                className="inline-flex items-center gap-1.5 rounded-lg px-4 py-2 text-sm font-semibold text-black shadow-md shadow-[var(--accent-glow)] transition-all [background:linear-gradient(to_bottom,var(--accent),var(--accent-deep))] hover:[background:linear-gradient(to_bottom,var(--accent-bright),var(--accent))] disabled:cursor-not-allowed disabled:opacity-45"
              >
                {importing && <Loader2 className="size-3.5 animate-spin" />}
                Import {selected.size === 1 ? "world" : `${selected.size} worlds`}
              </button>
            </div>
          </div>
        )}
    </Modal>
  );
}


function Chip({ children }: { children: React.ReactNode }) {
  return (
    <span className="shrink-0 rounded-md bg-surface-3 px-1.5 py-0.5 text-[10px] font-medium capitalize text-content-muted">
      {children}
    </span>
  );
}

function WorldCard({
  world,
  running,
  onOpenFolder,
  onDelete,
}: {
  world: WorldSummary;
  running: boolean;
  onOpenFolder: () => void;
  onDelete: () => Promise<void>;
}) {
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);

  const chips = [
    world.game_mode || null,
    difficultyLabel(world.difficulty),
    versionLabel(world.version_name, world.data_version),
  ].filter(Boolean) as string[];

  return (
    <div
      className={cn(
        "group relative flex min-w-0 gap-3.5 overflow-hidden rounded-2xl border bg-surface-2/60 p-3.5 transition-colors",
        world.status === "damaged"
          ? "border-danger/25"
          : "border-border-soft hover:border-border",
      )}
    >
      {world.icon_data_url ? (
        <img
          src={world.icon_data_url}
          alt=""
          className="size-16 shrink-0 rounded-xl bg-surface-3 object-cover [image-rendering:pixelated]"
          draggable={false}
        />
      ) : (
        <div className="grid size-16 shrink-0 place-items-center rounded-xl bg-surface-3 text-content-faint">
          <Globe2 className="size-6" />
        </div>
      )}

      <div className="flex min-w-0 flex-1 flex-col justify-center gap-1.5">
        <div className="flex min-w-0 items-center gap-2">
          <span className="truncate text-sm font-semibold text-content">{world.name}</span>
          {world.hardcore && (
            <span className="shrink-0 rounded-md bg-danger/15 px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-wide text-danger">
              Hardcore
            </span>
          )}
          <StatusBadge status={world.status} />
        </div>

        <div className="flex min-w-0 flex-wrap items-center gap-1">
          {chips.map((chip) => (
            <Chip key={chip}>{chip}</Chip>
          ))}
        </div>

        <div className="truncate text-[11px] text-content-faint">
          {world.error ? (
            <span className="text-danger">{world.error}</span>
          ) : (
            <>
              {world.last_played_ms != null
                ? `Played ${relativeTime(Math.floor(world.last_played_ms / 1000))}`
                : "Never played"}
              {" · "}
              <span className="font-mono">{world.folder_name}</span>
            </>
          )}
        </div>
      </div>

      <div className="absolute right-2.5 top-2.5 flex items-center gap-1 opacity-0 transition-opacity focus-within:opacity-100 group-hover:opacity-100">
        <button
          onClick={onOpenFolder}
          title="Open the world folder"
          aria-label="Open the world folder"
          className="grid size-8 place-items-center rounded-lg bg-surface-3/90 text-content-faint backdrop-blur transition-colors hover:bg-surface-3 hover:text-content"
        >
          <FolderOpen className="size-3.5" />
        </button>
        <button
          onClick={() => setConfirming(true)}
          disabled={running}
          title={running ? "Stop the game before deleting a world" : "Delete this world"}
          aria-label="Delete this world"
          className={cn(
            "grid size-8 place-items-center rounded-lg bg-surface-3/90 backdrop-blur transition-colors",
            running
              ? "cursor-not-allowed text-content-faint/40"
              : "text-content-faint hover:bg-danger hover:text-white",
          )}
        >
          <Trash2 className="size-3.5" />
        </button>
      </div>

      {confirming && (
        <div className="absolute inset-0 z-10 flex items-center gap-3 rounded-2xl border border-danger/40 bg-base/95 px-3.5 backdrop-blur">
          <TriangleAlert className="size-4 shrink-0 text-danger" />
          <div className="min-w-0 flex-1">
            <div className="truncate text-xs font-semibold text-content">
              Delete {world.name}?
            </div>
            <div className="text-[11px] text-content-faint">
              The save folder is removed from disk. This cannot be undone.
            </div>
          </div>
          <button
            onClick={() => setConfirming(false)}
            disabled={busy}
            className="shrink-0 rounded-lg px-2.5 py-1.5 text-xs font-medium text-content-muted transition-colors hover:text-content disabled:opacity-50"
          >
            Cancel
          </button>
          <button
            onClick={async () => {
              setBusy(true);
              try {
                await onDelete();
              } finally {
                setBusy(false);
                setConfirming(false);
              }
            }}
            disabled={busy}
            className="inline-flex shrink-0 items-center gap-1.5 rounded-lg border border-danger/40 bg-danger/15 px-2.5 py-1.5 text-xs font-semibold text-danger transition-colors hover:bg-danger/25 disabled:opacity-50"
          >
            {busy ? <Loader2 className="size-3.5 animate-spin" /> : <Trash2 className="size-3.5" />}
            Delete
          </button>
        </div>
      )}
    </div>
  );
}

export function WorldsPanel({
  instance,
  running,
  importOpen,
  onImportOpenChange,
  refreshToken,
  onLoadingChange,
}: {
  instance: Instance;
  running: boolean;
  importOpen: boolean;
  onImportOpenChange: (open: boolean) => void;
  refreshToken: number;
  onLoadingChange: (loading: boolean) => void;
}) {
  const [worlds, setWorlds] = useState<WorldSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [filter, setFilter] = useState("");

  const refresh = useCallback(async () => {
    setLoading(true);
    onLoadingChange(true);
    setError(null);
    try {
      setWorlds(await api.listInstanceWorlds(instance.id));
    } catch (cause) {
      setWorlds([]);
      setError(String(cause));
    } finally {
      setLoading(false);
      onLoadingChange(false);
    }
  }, [instance.id, onLoadingChange]);

  useEffect(() => {
    void refresh();
  }, [refresh, refreshToken]);

  useEffect(() => () => onLoadingChange(false), [onLoadingChange]);

  const remove = async (world: WorldSummary) => {
    setError(null);
    try {
      await api.deleteInstanceWorld(instance.id, world.folder_name);
      await refresh();
    } catch (cause) {
      setError(String(cause));
    }
  };

  const shownWorlds = useMemo(() => {
    const query = filter.trim().toLowerCase();
    if (!query) return worlds;
    return worlds.filter(
      (world) =>
        world.name.toLowerCase().includes(query) ||
        world.folder_name.toLowerCase().includes(query),
    );
  }, [filter, worlds]);

  return (
    <div className="px-6 py-5">
      <div className="mb-4 flex items-center gap-3">
        {worlds.length > 0 && (
          <div className="relative w-full max-w-sm">
            <Search className="absolute left-3 top-1/2 size-3.5 -translate-y-1/2 text-content-faint" />
            <input
              value={filter}
              onChange={(event) => setFilter(event.target.value)}
              placeholder="Filter worlds"
              className="w-full rounded-lg border border-border bg-base py-2 pl-9 pr-3 text-sm text-content outline-none transition-colors focus:border-[var(--accent)]"
            />
          </div>
        )}
        <div className="ml-auto flex items-center gap-2">
          {worlds.length > 0 && (
            <span className="mr-1 text-xs tabular-nums text-content-faint">
              {shownWorlds.length === worlds.length
                ? `${worlds.length} ${worlds.length === 1 ? "world" : "worlds"}`
                : `${shownWorlds.length} of ${worlds.length}`}
            </span>
          )}
        </div>
      </div>

      {error ? (
        <div className="flex gap-2.5 rounded-xl border border-danger/25 bg-danger/[0.07] px-4 py-3 text-sm text-danger">
          <TriangleAlert className="mt-0.5 size-4 shrink-0" />
          <span>{error}</span>
        </div>
      ) : loading && worlds.length === 0 ? (
        <div className="flex items-center justify-center gap-2 py-12 text-sm text-content-muted">
          <Loader2 className="size-4 animate-spin" />
          Loading worlds
        </div>
      ) : worlds.length === 0 ? (
        <div className="flex flex-col items-center gap-3 py-20 text-center">
          <div className="grid size-12 place-items-center rounded-2xl border border-border-soft bg-surface-2 text-content-faint">
            <Globe2 className="size-6" />
          </div>
          <div className="text-sm font-medium text-content-muted">No worlds yet</div>
          <p className="max-w-sm text-xs text-content-faint">
            Worlds created in-game appear here automatically. You can also import an
            extracted save or a ZIP backup.
          </p>
          <button
            onClick={() => onImportOpenChange(true)}
            className="mt-1 inline-flex items-center gap-1.5 rounded-lg px-3 py-2 text-xs font-semibold text-black shadow-md shadow-[var(--accent-glow)] transition-all [background:linear-gradient(to_bottom,var(--accent),var(--accent-deep))] hover:[background:linear-gradient(to_bottom,var(--accent-bright),var(--accent))]"
          >
            <HardDriveUpload className="size-3.5" />
            Import a world
          </button>
        </div>
      ) : shownWorlds.length === 0 ? (
        <div className="py-16 text-center text-sm text-content-faint">
          Nothing matches “{filter}”.
        </div>
      ) : (
        <div className="grid gap-3 lg:grid-cols-2">
          {shownWorlds.map((world) => (
            <WorldCard
              key={world.folder_name}
              world={world}
              running={running}
              onOpenFolder={() =>
                void openPath(`${instance.dir}/saves/${world.folder_name}`)
              }
              onDelete={() => remove(world)}
            />
          ))}
        </div>
      )}

      {importOpen && (
        <ImportDialog
          instance={instance}
          running={running}
          onClose={() => onImportOpenChange(false)}
          onImported={refresh}
        />
      )}
    </div>
  );
}
