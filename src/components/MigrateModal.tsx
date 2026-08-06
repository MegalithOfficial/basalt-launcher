import { useCallback, useEffect, useState } from "react";
import { motion } from "motion/react";
import {
  Boxes,
  Check,
  HardDriveDownload,
  Loader2,
  Package,
  TriangleAlert,
} from "lucide-react";

import { api } from "../lib/api";
import { cn } from "../lib/cn";
import { logoSrc } from "../lib/media";
import { LOADERS } from "../lib/loader";
import { relativeTime } from "../lib/time";
import type {
  Instance,
  LauncherSource,
  MigrationCandidate,
  MigrationOutcome,
} from "../lib/types";
import { useStore } from "../store";
import { Modal, ModalFooter, ModalHeader } from "./Modal";
import { formatBytes } from "../lib/format";

function shortPath(path: string): string {
  const home = path.match(/^(\/home\/[^/]+|\/Users\/[^/]+|[A-Z]:\\Users\\[^\\]+)/);
  return home ? `~${path.slice(home[0].length)}` : path;
}

function SourceTab({
  source,
  active,
  onSelect,
}: {
  source: LauncherSource;
  active: boolean;
  onSelect: () => void;
}) {
  return (
    <button
      onClick={onSelect}
      className={cn(
        "flex shrink-0 items-center gap-2 rounded-lg px-3 py-1.5 text-xs font-medium transition-colors",
        active
          ? "bg-surface-3 text-content"
          : "text-content-faint hover:text-content-muted",
      )}
    >
      {source.label}
      <span
        className={cn(
          "rounded px-1.5 text-[10px] font-semibold tabular-nums",
          active ? "bg-(--accent)/20 text-(--accent-bright)" : "bg-surface-3",
        )}
      >
        {source.instance_count}
      </span>
    </button>
  );
}


function CandidateRow({
  candidate,
  selected,
  onToggle,
}: {
  candidate: MigrationCandidate;
  selected: boolean;
  onToggle: () => void;
}) {
  return (
    <button
      type="button"
      disabled={!candidate.importable}
      onClick={onToggle}
      className={cn(
        "flex w-full items-center gap-3 rounded-lg px-2.5 py-2 text-left transition-colors",
        selected ? "bg-surface-2" : "hover:bg-surface-2/50",
        !candidate.importable && "cursor-not-allowed opacity-50",
      )}
    >
      <span
        className={cn(
          "grid size-4 shrink-0 place-items-center rounded border",
          selected ? "border-(--accent) bg-(--accent) text-black" : "border-border bg-surface-3",
        )}
      >
        {selected && <Check className="size-3" strokeWidth={3} />}
      </span>

      {candidate.icon_data_url ? (
        <img
          src={candidate.icon_data_url}
          alt=""
          className="size-9 shrink-0 rounded-lg bg-surface-3 object-cover"
          draggable={false}
        />
      ) : (
        <span className="grid size-9 shrink-0 place-items-center rounded-lg bg-surface-3 text-content-faint">
          <Package className="size-4" />
        </span>
      )}

      <span className="min-w-0 flex-1">
        <span className="flex min-w-0 items-center gap-2">
          <span className="truncate text-sm font-medium text-content">{candidate.name}</span>
          {candidate.pack && (
            <span className="shrink-0 rounded bg-surface-3 px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-wide text-content-faint">
              {candidate.pack === "modrinth" ? "Modrinth pack" : "CurseForge pack"}
            </span>
          )}
        </span>
        <span className="block truncate text-[11px] text-content-faint">
          {candidate.version_id || "unknown version"}
          {candidate.loader
            ? ` · ${LOADERS.find((l) => l.id === candidate.loader)?.label ?? candidate.loader}`
            : ""}
          {` · ${candidate.mod_count} mods · ${formatBytes(candidate.total_bytes)}`}
          {candidate.last_played_ms
            ? ` · played ${relativeTime(Math.floor(candidate.last_played_ms / 1000))}`
            : ""}
        </span>
        {candidate.imported && (
          <span className="mt-0.5 block text-[11px] text-ok">
            Already imported into Basalt
          </span>
        )}
        {candidate.warnings.map((warning) => (
          <span key={warning} className="mt-0.5 block text-[11px] text-warn">
            {warning}
          </span>
        ))}
      </span>
    </button>
  );
}

export function MigrateModal({ open, onClose }: { open: boolean; onClose: () => void }) {
  const refreshInstances = useStore((s) => s.refreshInstances);
  const instances = useStore((s) => s.instances);

  const [sources, setSources] = useState<LauncherSource[]>([]);
  const [source, setSource] = useState<LauncherSource | null>(null);
  const [candidates, setCandidates] = useState<MigrationCandidate[]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [scanning, setScanning] = useState(true);
  const [importing, setImporting] = useState(false);
  const [outcome, setOutcome] = useState<MigrationOutcome | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async (next: LauncherSource) => {
    setSource(next);
    setOutcome(null);
    const result = await api.scanLauncher(next.kind, next.root);
    setCandidates(result.candidates);
    setSelected(new Set());
  }, []);

  const scan = useCallback(async () => {
    setScanning(true);
    setError(null);
    setOutcome(null);
    try {
      const found = await api.detectLaunchers();
      setSources(found);
      const first = found[0] ?? null;
      setSource(first);
      if (!first) {
        setCandidates([]);
        return;
      }
      await load(first);
    } catch (cause) {
      setError(String(cause));
      setCandidates([]);
    } finally {
      setScanning(false);
    }
  }, [load]);

  useEffect(() => {
    if (open) void scan();
  }, [open, scan]);

  const toggle = (id: string) =>
    setSelected((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });

  const submit = async () => {
    if (!source || selected.size === 0) return;
    setImporting(true);
    setError(null);
    try {
      const result = await api.migrateInstances(source.kind, source.root, [...selected]);
      setOutcome(result);
      await refreshInstances();
    } catch (cause) {
      setError(String(cause));
    } finally {
      setImporting(false);
    }
  };

  const imported = outcome
    ? outcome.imported
        .map((id) => instances.find((instance) => instance.id === id))
        .filter((instance): instance is Instance => !!instance)
    : [];

  const totalBytes = candidates
    .filter((c) => selected.has(c.id))
    .reduce((sum, c) => sum + c.total_bytes, 0);

  return (
    <Modal
      open={open}
      onClose={onClose}
      size="wide"
      className="h-[min(640px,calc(100vh-48px))]"
      dismissable={!importing}
      labelledBy="migrate-title"
    >
      <ModalHeader
        id="migrate-title"
        title="Import from another launcher"
        subtitle="Instances are copied, so the other launcher keeps its own."
        icon={
          <div className="grid size-9 place-items-center rounded-xl border border-border-soft bg-surface-2 text-(--accent)">
            <HardDriveDownload className="size-4" />
          </div>
        }
        onClose={importing ? undefined : onClose}
      />

      <div className="flex min-h-0 flex-1 flex-col px-5 py-4">
        {scanning && (
          <div className="flex flex-col items-center gap-3 py-14 text-center">
            <Loader2 className="size-5 animate-spin text-(--accent)" />
            <div className="text-sm font-medium text-content">Looking for launchers</div>
          </div>
        )}

        {!scanning && !source && (
          <div className="flex flex-col items-center gap-3 py-14 text-center">
            <div className="grid size-12 place-items-center rounded-2xl border border-border-soft bg-surface-2 text-content-faint">
              <Boxes className="size-6" />
            </div>
            <div className="text-sm font-medium text-content-muted">No other launcher found</div>
            <p className="max-w-sm text-xs text-content-faint">
              ATLauncher, Prism Launcher and the Modrinth App are detected in their default
              data folders. Nothing was found there.
            </p>
          </div>
        )}

        {!scanning && source && outcome === null && (
          <>
            {sources.length > 0 && (
              <div className="mb-3 flex shrink-0 items-center gap-3">
                <div className="flex shrink-0 items-center gap-1 rounded-lg border border-border-soft bg-surface-2/60 p-1">
                  {sources.map((entry) => (
                    <SourceTab
                      key={entry.root}
                      source={entry}
                      active={entry.root === source.root}
                      onSelect={() => void load(entry)}
                    />
                  ))}
                </div>
                <span className="min-w-0 flex-1 truncate text-right font-mono text-[10px] text-content-faint">
                  {shortPath(source.root)}
                </span>
              </div>
            )}

            <div className="mb-2 flex shrink-0 items-center justify-between gap-3">
              <span className="text-[11px] text-content-faint">
                Minecraft and the loader are downloaded the first time you play.
              </span>
              <button
                onClick={() =>
                  setSelected(
                    selected.size > 0
                      ? new Set()
                      : new Set(candidates.filter((c) => c.importable).map((c) => c.id)),
                  )
                }
                className="shrink-0 text-[11px] font-medium text-content-muted transition-colors hover:text-content"
              >
                {selected.size > 0 ? "Clear" : "Select all"}
              </button>
            </div>
            <div className="-mx-1 min-h-0 flex-1 overflow-y-auto px-1">
              <div className="flex flex-col gap-0.5">
              {candidates.map((candidate) => (
                <CandidateRow
                  key={candidate.id}
                  candidate={candidate}
                  selected={selected.has(candidate.id)}
                  onToggle={() => toggle(candidate.id)}
                />
              ))}
              </div>
            </div>
          </>
        )}

        {outcome && (
          <div className="flex h-full flex-col items-center justify-center gap-5 py-6 text-center">
            <motion.span
              initial={{ scale: 0.6, opacity: 0 }}
              animate={{ scale: 1, opacity: 1 }}
              transition={{ type: "spring", stiffness: 220, damping: 16 }}
              className="grid size-16 place-items-center rounded-2xl bg-ok/15 text-ok"
            >
              <Check className="size-8" strokeWidth={2.5} />
            </motion.span>

            <div>
              <div className="font-display text-xl font-semibold text-content">
                {outcome.imported.length === 1
                  ? "1 instance imported"
                  : `${outcome.imported.length} instances imported`}
              </div>
              <p className="mt-1 text-xs text-content-muted">
                They are ready to play. Minecraft downloads on the first launch.
              </p>
            </div>

            {imported.length > 0 && (
              <div className="flex w-full max-w-sm flex-col gap-1.5">
                {imported.map((instance) => (
                  <div
                    key={instance.id}
                    className="flex items-center gap-3 rounded-xl border border-border-soft bg-surface-2/60 px-3 py-2 text-left"
                  >
                    {logoSrc(instance.logo) ? (
                      <img
                        src={logoSrc(instance.logo) as string}
                        alt=""
                        className="size-8 shrink-0 rounded-lg bg-surface-3 object-cover"
                        draggable={false}
                      />
                    ) : (
                      <span className="grid size-8 shrink-0 place-items-center rounded-lg bg-surface-3 text-content-faint">
                        <Package className="size-4" />
                      </span>
                    )}
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-[13px] font-medium text-content">
                        {instance.name}
                      </span>
                      <span className="block truncate text-[11px] text-content-faint">
                        {instance.version_id}
                        {instance.loader ? ` · ${instance.loader}` : ""}
                      </span>
                    </span>
                  </div>
                ))}
              </div>
            )}

            {outcome.failed.length > 0 && (
              <div className="flex w-full max-w-sm flex-col gap-1.5 text-left">
                {outcome.failed.map(([id, reason]) => (
                  <div
                    key={id}
                    className="flex gap-2.5 rounded-xl border border-danger/25 bg-danger/[0.07] px-3.5 py-2.5 text-xs text-danger"
                  >
                    <TriangleAlert className="mt-0.5 size-4 shrink-0" />
                    <span className="wrap-break-word">
                      <span className="font-medium">{id}</span> · {reason}
                    </span>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}

        {error && (
          <div className="mt-4 flex gap-2.5 rounded-xl border border-danger/25 bg-danger/[0.07] px-3.5 py-3 text-xs text-danger">
            <TriangleAlert className="mt-0.5 size-4 shrink-0" />
            <span className="wrap-break-word">{error}</span>
          </div>
        )}
      </div>

      {!scanning && source && (
        <ModalFooter className="justify-between">
          <span className="text-xs text-content-faint">
            {outcome ? "" : `${selected.size} selected · ${formatBytes(totalBytes)} to copy`}
          </span>
          <div className="flex items-center gap-2">
            <button
              onClick={onClose}
              disabled={importing}
              className="rounded-lg px-3 py-2 text-sm font-medium text-content-muted transition-colors hover:text-content disabled:opacity-50"
            >
              {outcome ? "Done" : "Cancel"}
            </button>
            {!outcome && (
              <button
                onClick={submit}
                disabled={selected.size === 0 || importing}
                className="inline-flex items-center gap-1.5 rounded-lg px-4 py-2 text-sm font-semibold text-black shadow-md shadow-(color:--accent-glow) transition-all [background:linear-gradient(to_bottom,var(--accent),var(--accent-deep))] hover:[background:linear-gradient(to_bottom,var(--accent-bright),var(--accent))] disabled:cursor-not-allowed disabled:opacity-45"
              >
                {importing && <Loader2 className="size-3.5 animate-spin" />}
                Import {selected.size === 1 ? "instance" : `${selected.size} instances`}
              </button>
            )}
          </div>
        </ModalFooter>
      )}
    </Modal>
  );
}
