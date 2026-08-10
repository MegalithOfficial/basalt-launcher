import { useCallback, useEffect, useMemo, useState } from "react";
import {
  ChevronRight,
  FolderOpen,
  Info,
  Loader2,
  RefreshCw,
  Trash2,
  TriangleAlert,
} from "lucide-react";
import { toast } from "sonner";

import { api } from "../../lib/api";
import { cn } from "../../lib/cn";
import { log } from "../../lib/log";
import { openFolder } from "../../lib/reveal";
import { formatBytes } from "../../lib/format";
import type { Reclaimable, StorageEntry, StorageReport } from "../../lib/types";
import { ConfirmDialog } from "../ConfirmDialog";
import { DataLocations } from "./DataLocations";

const BUCKET_COLORS: Record<string, string> = {
  instances: "var(--accent)",
  shared: "var(--color-ok)",
  caches: "var(--color-warn)",
  snapshots: "var(--color-lava-deep)",
  database: "var(--color-content-faint)",
  media: "var(--color-ember)",
  logs: "var(--color-border)",
  partials: "var(--color-danger)",
};

function colorOf(id: string) {
  return BUCKET_COLORS[id] ?? "var(--color-surface-3)";
}

function share(bytes: number, total: number) {
  if (total <= 0 || bytes <= 0) return "";
  const percent = (bytes / total) * 100;
  return percent < 1 ? "<1%" : `${Math.round(percent)}%`;
}

function OpenButton({ path, label }: { path: string; label: string }) {
  return (
    <button
      onClick={(event) => {
        event.stopPropagation();
        openFolder(path);
      }}
      title={`Open ${label}`}
      aria-label={`Open ${label}`}
      className="grid size-6 shrink-0 place-items-center rounded-md text-content-faint opacity-0 transition-colors hover:bg-surface-3 hover:text-content focus-visible:opacity-100 group-hover/row:opacity-100"
    >
      <FolderOpen className="size-3.5" />
    </button>
  );
}

const TAIL_SHARE = 0.01;
const TAIL_MINIMUM = 4;

function splitTail(entries: StorageEntry[], total: number) {
  if (total <= 0) return { head: entries, tail: [] as StorageEntry[] };
  const tail = entries.filter((entry) => entry.bytes / total < TAIL_SHARE);
  if (tail.length < TAIL_MINIMUM) return { head: entries, tail: [] as StorageEntry[] };
  return { head: entries.filter((entry) => !tail.includes(entry)), tail };
}

function Row({
  entry,
  total,
  depth,
  showShare,
}: {
  entry: StorageEntry;
  total: number;
  depth: number;
  showShare: boolean;
}) {
  const [open, setOpen] = useState(false);
  const [tailOpen, setTailOpen] = useState(false);
  const expandable = entry.children.length > 0;
  const { head, tail } = splitTail(entry.children, entry.bytes);
  const tailBytes = tail.reduce((sum, item) => sum + item.bytes, 0);

  return (
    <div className={cn(depth === 0 && "border-b border-border-soft/60 last:border-b-0")}>
      <div className="group/row flex items-center gap-3" style={{ paddingLeft: depth * 20 }}>
        <button
          onClick={() => expandable && setOpen((value) => !value)}
          className={cn(
            "flex min-w-0 flex-1 items-center gap-2.5 py-1.5 text-left",
            depth === 0 && "py-2",
            expandable ? "cursor-pointer" : "cursor-default",
          )}
        >
          <ChevronRight
            className={cn(
              "size-3.5 shrink-0 transition-transform",
              expandable ? "text-content-faint" : "text-transparent",
              open && "rotate-90",
            )}
          />
          {depth === 0 && (
            <span
              className="size-2.5 shrink-0 rounded-[3px]"
              style={{ background: colorOf(entry.id) }}
            />
          )}
          <span
            className={cn(
              "truncate",
              depth === 0 && "text-sm font-medium text-content",
              depth === 1 && "text-[13px] text-content",
              depth >= 2 && "font-mono text-[11px] text-content-muted",
            )}
          >
            {entry.label}
          </span>
        </button>
        {showShare && (
          <span className="w-9 shrink-0 text-right tabular-nums text-[11px] text-content-faint">
            {share(entry.bytes, total)}
          </span>
        )}
        <span
          className={cn(
            "w-20 shrink-0 text-right tabular-nums",
            depth === 0 ? "text-sm text-content-muted" : "text-[12px] text-content-faint",
          )}
        >
          {formatBytes(entry.bytes)}
        </span>
        <span className="w-6 shrink-0">
          {entry.path && <OpenButton path={entry.path} label={entry.label} />}
        </span>
      </div>

      {open && (
        <div className="pb-1">
          {head.map((child) => (
            <Row
              key={child.id}
              entry={child}
              total={entry.bytes}
              depth={depth + 1}
              showShare={false}
            />
          ))}
          {tail.length > 0 && (
            <>
              <button
                onClick={() => setTailOpen((value) => !value)}
                className="flex w-full items-center gap-2.5 py-1.5 text-left"
                style={{ paddingLeft: (depth + 1) * 20 }}
              >
                <ChevronRight
                  className={cn(
                    "size-3.5 shrink-0 text-content-faint transition-transform",
                    tailOpen && "rotate-90",
                  )}
                />
                <span className="min-w-0 flex-1 truncate text-[11px] text-content-faint">
                  {tail.length} smaller folders
                </span>
                <span className="w-20 shrink-0 text-right text-[12px] tabular-nums text-content-faint">
                  {formatBytes(tailBytes)}
                </span>
                <span className="w-6 shrink-0" />
              </button>
              {tailOpen &&
                tail.map((child) => (
                  <Row
                    key={child.id}
                    entry={child}
                    total={entry.bytes}
                    depth={depth + 2}
                    showShare={false}
                  />
                ))}
            </>
          )}
        </div>
      )}
    </div>
  );
}

function ReclaimRow({
  entry,
  checked,
  onToggle,
}: {
  entry: Reclaimable;
  checked: boolean;
  onToggle: () => void;
}) {
  return (
    <label className="flex cursor-pointer items-start gap-3 border-b border-border-soft/60 py-2.5 last:border-b-0">
      <input
        type="checkbox"
        checked={checked}
        onChange={onToggle}
        className="mt-1 size-3.5 shrink-0 accent-(--accent)"
      />
      <span className="min-w-0 flex-1">
        <span className="flex items-baseline gap-2">
          <span className="text-sm font-medium text-content">{entry.label}</span>
          {entry.count > 0 && (
            <span className="text-[11px] text-content-faint">
              {entry.count} {entry.count === 1 ? "item" : "items"}
            </span>
          )}
        </span>
        <span className="mt-0.5 block text-[11px] leading-relaxed text-content-muted">
          {entry.detail}
        </span>
        {entry.items.length > 0 && (
          <span className="mt-1.5 flex flex-wrap gap-1.5">
            {entry.items.map((item) => (
              <span
                key={item}
                className="rounded border border-border bg-surface-3 px-1.5 py-0.5 font-mono text-[10px] text-content-muted"
              >
                {item}
              </span>
            ))}
          </span>
        )}
      </span>
      <span className="w-20 shrink-0 text-right text-sm tabular-nums text-content-muted">
        {formatBytes(entry.bytes)}
      </span>
    </label>
  );
}

function Group({
  title,
  description,
  entries,
  chosen,
  onToggle,
  onAll,
}: {
  title: string;
  description: string;
  entries: Reclaimable[];
  chosen: Set<string>;
  onToggle: (id: string) => void;
  onAll: (ids: string[], all: boolean) => void;
}) {
  if (entries.length === 0) return null;

  const ids = entries.map((entry) => entry.id);
  const all = ids.every((id) => chosen.has(id));
  const bytes = entries.reduce((sum, entry) => sum + entry.bytes, 0);

  return (
    <div className="mt-5">
      <div className="flex items-baseline gap-3">
        <h4 className="text-[13px] font-semibold text-content">{title}</h4>
        <button
          onClick={() => onAll(ids, all)}
          className="text-[11px] font-medium text-(--accent) transition-opacity hover:opacity-80"
        >
          {all ? "Clear selection" : "Select all"}
        </button>
        <span className="flex-1" />
        <span className="text-[13px] tabular-nums text-content-faint">{formatBytes(bytes)}</span>
      </div>
      <p className="mt-0.5 text-[11px] text-content-muted">{description}</p>
      <div className="mt-1.5">
        {entries.map((entry) => (
          <ReclaimRow
            key={entry.id}
            entry={entry}
            checked={chosen.has(entry.id)}
            onToggle={() => onToggle(entry.id)}
          />
        ))}
      </div>
    </div>
  );
}

function Shimmer({ className }: { className?: string }) {
  return <div className={cn("animate-pulse rounded bg-surface-3/50", className)} />;
}

const SKELETON_ROWS = [
  { width: "w-28", child: true },
  { width: "w-36", child: false },
  { width: "w-24", child: false },
  { width: "w-20", child: false },
  { width: "w-32", child: false },
];

function StorageSkeleton() {
  return (
    <div className="pb-6" aria-busy="true" aria-label="Measuring storage">
      <div className="mb-4 flex items-end gap-3">
        <div className="flex-1">
          <Shimmer className="h-6 w-40" />
          <Shimmer className="mt-2 h-3 w-56" />
        </div>
        <Shimmer className="h-8 w-24 rounded-lg" />
      </div>

      <div className="mb-5 flex h-2.5 w-full animate-pulse overflow-hidden rounded-full bg-surface-2">
        <span className="h-full w-[62%] bg-surface-3/60" />
        <span className="h-full w-[18%] bg-surface-3/40" />
        <span className="h-full w-[9%] bg-surface-3/60" />
        <span className="h-full flex-1 bg-surface-3/30" />
      </div>

      <div className="mb-8">
        {SKELETON_ROWS.map((row, index) => (
          <div key={index} className="border-b border-border-soft/60 last:border-b-0">
            <div className="flex items-center gap-3 py-2">
              <Shimmer className="size-3.5 shrink-0 rounded-sm" />
              <Shimmer className="size-2.5 shrink-0 rounded-[3px]" />
              <Shimmer className={cn("h-3.5", row.width)} />
              <span className="flex-1" />
              <Shimmer className="h-3 w-7 shrink-0" />
              <Shimmer className="h-3.5 w-16 shrink-0" />
              <Shimmer className="h-6 w-16 shrink-0 rounded-lg" />
            </div>
            {row.child && (
              <div className="pb-2 pl-8">
                {["w-20", "w-24", "w-16"].map((width) => (
                  <div key={width} className="flex items-center gap-3 py-1.5">
                    <Shimmer className={cn("h-3", width)} />
                    <span className="flex-1" />
                    <Shimmer className="h-3 w-14 shrink-0" />
                    <Shimmer className="h-6 w-16 shrink-0 rounded-lg" />
                  </div>
                ))}
              </div>
            )}
          </div>
        ))}
      </div>

      <Shimmer className="h-4 w-32" />
      <Shimmer className="mt-2 h-3 w-80" />
      <div className="mt-3">
        {["w-32", "w-40", "w-28"].map((width) => (
          <div
            key={width}
            className="flex items-start gap-3 border-b border-border-soft/60 py-2.5 last:border-b-0"
          >
            <Shimmer className="mt-1 size-3.5 shrink-0 rounded-sm" />
            <div className="min-w-0 flex-1">
              <Shimmer className={cn("h-3.5", width)} />
              <Shimmer className="mt-1.5 h-3 w-72" />
            </div>
            <Shimmer className="h-3.5 w-16 shrink-0" />
          </div>
        ))}
      </div>
      <Shimmer className="mt-4 h-8 w-32 rounded-lg" />
    </div>
  );
}

export function StoragePanel() {
  const [report, setReport] = useState<StorageReport | null>(null);
  const [scanning, setScanning] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [picked, setPicked] = useState<string[]>([]);
  const [confirming, setConfirming] = useState(false);
  const [clearing, setClearing] = useState(false);
  const [showSpare, setShowSpare] = useState(false);

  const load = useCallback(async (force: boolean) => {
    setScanning(true);
    setError(null);
    try {
      setReport(await api.scanStorage(force));
    } catch (cause) {
      log.warn("storage", `could not measure storage: ${String(cause)}`);
      setError(String(cause));
    } finally {
      setScanning(false);
    }
  }, []);

  useEffect(() => {
    void load(false);
  }, [load]);

  const chosen = useMemo(() => new Set(picked), [picked]);
  const offered = report?.reclaimable ?? [];
  const caches = offered.filter((entry) => entry.tier === "cache");
  const unused = offered.filter((entry) => entry.tier === "shared");
  const spare = offered.filter((entry) => entry.tier === "spare");
  const offeredBytes = offered.reduce((sum, entry) => sum + entry.bytes, 0);
  const selectedBytes = offered
    .filter((entry) => chosen.has(entry.id))
    .reduce((sum, entry) => sum + entry.bytes, 0);

  const toggle = (id: string) =>
    setPicked((current) =>
      current.includes(id) ? current.filter((value) => value !== id) : [...current, id],
    );

  const selectGroup = (ids: string[], all: boolean) =>
    setPicked((current) => {
      const without = current.filter((id) => !ids.includes(id));
      return all ? without : [...without, ...ids];
    });

  const reclaim = async () => {
    setClearing(true);
    try {
      const outcome = await api.reclaimStorage(picked);
      if (outcome.failures.length > 0) {
        toast.warning(`Freed ${formatBytes(outcome.freed_bytes)}, with some left behind`, {
          description: outcome.failures.join("\n"),
        });
      } else {
        toast.success(`Freed ${formatBytes(outcome.freed_bytes)}`);
      }
      setPicked([]);
      setConfirming(false);
      await load(true);
    } catch (cause) {
      toast.error("Nothing was removed", { description: String(cause) });
    } finally {
      setClearing(false);
    }
  };

  if (scanning && !report) {
    return <StorageSkeleton />;
  }

  if (error && !report) {
    return <div className="py-10 text-sm text-danger">{error}</div>;
  }

  if (!report) return null;

  const total = report.total_bytes;

  return (
    <div className="pb-6">
      <div className="mb-4 flex flex-wrap items-end gap-3">
        <div className="min-w-0 flex-1">
          <p className="font-display text-xl font-semibold text-content">
            {formatBytes(total)} in use
          </p>
          <p className="mt-0.5 text-xs text-content-muted">
            {report.free_bytes != null
              ? `${formatBytes(report.free_bytes)} free on this drive`
              : "Free space is unknown on this drive"}
          </p>
        </div>
        <button
          onClick={() => void load(true)}
          disabled={scanning}
          className="inline-flex shrink-0 items-center gap-1.5 rounded-lg border border-border bg-surface-2 px-3 py-2 text-xs font-medium text-content-muted transition-colors hover:bg-surface-3 hover:text-content disabled:opacity-50"
        >
          <RefreshCw className={cn("size-3.5", scanning && "animate-spin")} />
          Rescan
        </button>
      </div>

      <div className="mb-5 flex h-2.5 w-full overflow-hidden rounded-full bg-surface-2">
        {report.buckets.map((bucket) => (
          <span
            key={bucket.id}
            title={`${bucket.label} · ${formatBytes(bucket.bytes)}`}
            style={{
              width: `${(bucket.bytes / Math.max(total, 1)) * 100}%`,
              background: colorOf(bucket.id),
            }}
          />
        ))}
      </div>

      <div className="mb-8">
        {report.buckets.map((bucket) => (
          <Row key={bucket.id} entry={bucket} total={total} depth={0} showShare />
        ))}
      </div>

      <div className="mb-8">
        <DataLocations />
      </div>

      <div className="flex items-baseline gap-3">
        <h3 className="font-display text-base font-semibold text-content">Reclaim space</h3>
        <span className="text-xs text-content-faint">
          {formatBytes(offeredBytes)} available
        </span>
      </div>

      {offered.length === 0 ? (
        <p className="mt-3 text-sm text-content-faint">There is nothing to clear right now.</p>
      ) : (
        <>
          <Group
            title="Caches"
            description="Basalt downloads or rebuilds these the moment it needs them again."
            entries={caches}
            chosen={chosen}
            onToggle={toggle}
            onAll={selectGroup}
          />
          <Group
            title="Unused game files"
            description="No instance on your list can reach these. Repair fetches back anything you turn out to need."
            entries={unused}
            chosen={chosen}
            onToggle={toggle}
            onAll={selectGroup}
          />

          {spare.length > 0 && (
            <div className="mt-5">
              <button
                onClick={() => setShowSpare((value) => !value)}
                className="flex w-full items-center gap-2 text-left"
              >
                <ChevronRight
                  className={cn(
                    "size-3.5 shrink-0 text-content-faint transition-transform",
                    showSpare && "rotate-90",
                  )}
                />
                <span className="text-[13px] font-medium text-content-muted">
                  Loader versions nothing is on
                </span>
                <span className="flex-1" />
                <span className="text-[13px] tabular-nums text-content-faint">
                  {formatBytes(spare.reduce((sum, entry) => sum + entry.bytes, 0))}
                </span>
              </button>
              {showSpare && (
                <div className="mt-1">
                  {spare.map((entry) => (
                    <ReclaimRow
                      key={entry.id}
                      entry={entry}
                      checked={chosen.has(entry.id)}
                      onToggle={() => toggle(entry.id)}
                    />
                  ))}
                </div>
              )}
            </div>
          )}
        </>
      )}

      {report.unresolved && (
        <div className="mt-4 flex gap-2.5 rounded-xl border border-border-soft bg-surface-2/50 px-3.5 py-3 text-[11px] text-content-muted">
          <Info className="mt-0.5 size-4 shrink-0 text-content-faint" />
          <span>Basalt is not offering to remove unused game files. {report.unresolved}</span>
        </div>
      )}

      <div className="mt-4 flex flex-wrap items-center gap-3">
        <button
          onClick={() => setConfirming(true)}
          disabled={picked.length === 0 || clearing}
          className="inline-flex items-center gap-1.5 rounded-lg border border-danger/40 bg-danger/10 px-3 py-2 text-xs font-semibold text-danger transition-colors hover:bg-danger/20 disabled:cursor-not-allowed disabled:opacity-40"
        >
          {clearing ? <Loader2 className="size-3.5 animate-spin" /> : <Trash2 className="size-3.5" />}
          {picked.length === 0 ? "Nothing selected" : `Free ${formatBytes(selectedBytes)}`}
        </button>
        {!report.shared_dedupe && (
          <span className="inline-flex items-center gap-1.5 text-[11px] text-content-faint">
            <TriangleAlert className="size-3.5" />
            Files shared between instances are counted once per instance on this system.
          </span>
        )}
      </div>

      <ConfirmDialog
        open={confirming}
        title={`Free ${formatBytes(selectedBytes)}?`}
        description="The files below are removed from disk. Basalt fetches them again when something needs them."
        confirmLabel="Free space"
        confirmIcon={<Trash2 className="size-3.5" />}
        onConfirm={() => void reclaim()}
        onCancel={() => setConfirming(false)}
      >
        <ul className="mt-3 flex flex-col gap-1">
          {offered
            .filter((entry) => chosen.has(entry.id))
            .map((entry) => (
              <li key={entry.id} className="flex items-baseline gap-2 text-xs text-content-muted">
                <span className="min-w-0 flex-1">{entry.label}</span>
                <span className="shrink-0 tabular-nums text-content-faint">
                  {formatBytes(entry.bytes)}
                </span>
              </li>
            ))}
        </ul>
      </ConfirmDialog>
    </div>
  );
}
