import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useVirtualizer } from "@tanstack/react-virtual";
import { convertFileSrc } from "@tauri-apps/api/core";
import {
  Camera,
  Check,
  ChevronLeft,
  ChevronRight,
  Copy,
  ExternalLink,
  FolderOpen,
  Trash2,
  X,
} from "lucide-react";
import { toast } from "sonner";

import { api } from "../../lib/api";
import { cn } from "../../lib/cn";
import { log } from "../../lib/log";
import { notifyRemoved } from "../../lib/notify";
import { openFile, openFolder } from "../../lib/reveal";
import type { Instance, Screenshot } from "../../lib/types";
import { ConfirmDialog } from "../ConfirmDialog";
import { Button, EmptyState } from "../ui";
import { formatBytes } from "../../lib/format";
import { formatDateTime } from "../../lib/time";

const GAP = 12;
const MIN_CARD = 230;
const FOOTER = 38;



function Shimmer({ className }: { className?: string }) {
  return <div className={cn("animate-pulse rounded bg-surface-3/50", className)} />;
}

function ScreenshotsSkeleton() {
  return (
    <div className="flex min-h-0 flex-1 flex-col" aria-busy="true" aria-label="Reading the folder">
      <div className="flex items-center gap-2 px-6 py-3">
        <Shimmer className="h-3.5 w-28" />
        <span className="flex-1" />
        <Shimmer className="h-7 w-20 rounded-lg" />
      </div>
      <div className="grid grid-cols-[repeat(auto-fill,minmax(230px,1fr))] gap-3 px-6">
        {Array.from({ length: 8 }, (_, index) => (
          <div key={index} className="overflow-hidden rounded-xl border border-border-soft">
            <div className="aspect-video w-full animate-pulse bg-surface-3/50" />
            <div className="flex items-center gap-2 px-3 py-2">
              <Shimmer className="h-3 flex-1" />
              <Shimmer className="h-3 w-10" />
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

export function ScreenshotsPanel({ instance }: { instance: Instance }) {
  const [shots, setShots] = useState<Screenshot[]>([]);
  const [loading, setLoading] = useState(true);
  const [picked, setPicked] = useState<string[]>([]);
  const [viewing, setViewing] = useState<number | null>(null);
  const [confirming, setConfirming] = useState(false);
  const [previews, setPreviews] = useState<Record<string, string>>({});
  const attempted = useRef(new Set<string>());
  const showing = useRef(instance.id);
  showing.current = instance.id;

  const refresh = useCallback(async () => {
    try {
      const listed = await api.listScreenshots(instance.id);
      setShots(listed);
      const cached = listed.filter((shot) => shot.thumbnail);
      attempted.current = new Set(cached.map((shot) => shot.name));
      setPreviews(
        Object.fromEntries(
          cached.map((shot) => [shot.name, convertFileSrc(shot.thumbnail as string)]),
        ),
      );
    } catch (cause) {
      log.warn("screenshots", `could not list screenshots: ${String(cause)}`);
      setShots([]);
    } finally {
      setLoading(false);
    }
  }, [instance.id]);

  useEffect(() => {
    setLoading(true);
    setPicked([]);
    setViewing(null);
    void refresh();
  }, [refresh]);

  const chosen = useMemo(() => new Set(picked), [picked]);

  const [scroller, setScroller] = useState<HTMLDivElement | null>(null);
  const [columns, setColumns] = useState(1);
  const [rowHeight, setRowHeight] = useState(MIN_CARD * (9 / 16) + FOOTER);

  useEffect(() => {
    if (!scroller) return;
    const measure = () => {
      const inner = scroller.clientWidth - 48;
      if (inner <= 0) return;
      const fitting = Math.max(1, Math.floor((inner + GAP) / (MIN_CARD + GAP)));
      setColumns(fitting);
      setRowHeight((inner - GAP * (fitting - 1)) / fitting / (16 / 9) + FOOTER);
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(scroller);
    return () => observer.disconnect();
  }, [scroller]);

  const rows = Math.ceil(shots.length / columns);

  const virtualizer = useVirtualizer({
    count: rows,
    getScrollElement: () => scroller,
    estimateSize: () => rowHeight,
    overscan: 1,
    gap: GAP,
  });

  const measureRows = virtualizer.measure;
  useEffect(() => {
    measureRows();
  }, [measureRows, columns, rowHeight]);

  const visible = virtualizer.getVirtualItems();
  const wanted = useMemo(() => {
    if (visible.length === 0) return "";
    const first = visible[0].index * columns;
    const last = visible[visible.length - 1].index * columns + columns;
    return shots
      .slice(first, last)
      .map((shot) => shot.name)
      .join("\n");
  }, [visible, shots, columns]);

  useEffect(() => {
    if (!wanted) return;
    const missing = wanted.split("\n").filter((name) => name && !attempted.current.has(name));
    if (missing.length === 0) return;
    missing.forEach((name) => attempted.current.add(name));

    const target = instance.id;
    api
      .ensureThumbnails(target, missing)
      .then((built) => {
        if (showing.current !== target) return;
        setPreviews((current) => ({
          ...current,
          ...Object.fromEntries(
            built
              .filter((entry) => entry.path)
              .map((entry) => [entry.name, convertFileSrc(entry.path as string)]),
          ),
        }));
      })
      .catch((cause) => log.warn("screenshots", `no previews: ${String(cause)}`));
  }, [wanted, instance.id]);

  const open = viewing != null ? shots[viewing] : null;

  useEffect(() => {
    if (viewing == null) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") setViewing(null);
      if (event.key === "ArrowLeft") setViewing((at) => (at == null ? at : Math.max(0, at - 1)));
      if (event.key === "ArrowRight")
        setViewing((at) => (at == null ? at : Math.min(shots.length - 1, at + 1)));
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [viewing, shots.length]);

  const toggle = (name: string) =>
    setPicked((current) =>
      current.includes(name) ? current.filter((value) => value !== name) : [...current, name],
    );

  const copy = async (shot: Screenshot) => {
    try {
      await api.copyScreenshot(instance.id, shot.name);
      toast.success("Copied to the clipboard", { description: shot.name });
    } catch (cause) {
      toast.error("Could not copy that", { description: String(cause) });
    }
  };

  const remove = async () => {
    const names = picked.length > 0 ? picked : open ? [open.name] : [];
    if (names.length === 0) return;
    try {
      await api.deleteScreenshots(instance.id, names);
      notifyRemoved(
        names.length === 1 ? `Deleted ${names[0]}` : `Deleted ${names.length} screenshots`,
        instance.name,
      );
      setPicked([]);
      setViewing(null);
      setConfirming(false);
      await refresh();
    } catch (cause) {
      toast.error("Could not delete that", { description: String(cause) });
    }
  };

  if (loading) {
    return <ScreenshotsSkeleton />;
  }

  if (shots.length === 0) {
    return (
      <EmptyState
        icon={<Camera className="size-6" />}
        title="No screenshots yet"
        description="Press F2 in game and every shot lands here, ready to copy, open or delete."
        action={
          <Button variant="ghost" onClick={() => openFolder(`${instance.dir}/screenshots`)}>
            <FolderOpen className="size-4" />
            Open the folder
          </Button>
        }
      />
    );
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex flex-wrap items-center gap-2 px-6 py-3">
        <span className="text-xs text-content-muted">
          {picked.length > 0
            ? `${picked.length} of ${shots.length} selected`
            : `${shots.length} ${shots.length === 1 ? "screenshot" : "screenshots"}`}
        </span>

        <span className="flex-1" />

        {picked.length > 0 && (
          <>
            <button
              onClick={() => setPicked([])}
              className="rounded-lg px-2.5 py-1.5 text-[11px] font-medium text-content-muted transition-colors hover:text-content"
            >
              Clear
            </button>
            <button
              onClick={() => setConfirming(true)}
              className="inline-flex items-center gap-1.5 rounded-lg border border-danger/40 bg-danger/10 px-2.5 py-1.5 text-[11px] font-semibold text-danger transition-colors hover:bg-danger/20"
            >
              <Trash2 className="size-3.5" />
              Delete {picked.length}
            </button>
          </>
        )}

        <button
          onClick={() => openFolder(`${instance.dir}/screenshots`)}
          className="inline-flex items-center gap-1.5 rounded-lg border border-border bg-surface-2 px-2.5 py-1.5 text-[11px] font-medium text-content-muted transition-colors hover:bg-surface-3 hover:text-content"
        >
          <FolderOpen className="size-3.5" />
          Folder
        </button>
      </div>

      <div ref={setScroller} className="min-h-0 flex-1 overflow-y-auto px-6 pb-6">
        <div className="relative w-full" style={{ height: virtualizer.getTotalSize() }}>
          {visible.map((row) => (
            <div
              key={row.key}
              data-index={row.index}
              ref={virtualizer.measureElement}
              className="absolute left-0 top-0 grid w-full"
              style={{
                transform: `translateY(${row.start}px)`,
                gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))`,
                gap: GAP,
              }}
            >
              {shots.slice(row.index * columns, row.index * columns + columns).map((shot, cell) => {
                const index = row.index * columns + cell;
                const selected = chosen.has(shot.name);
                return (
                  <div
                    key={shot.name}
                    className={cn(
                      "group relative overflow-hidden rounded-xl border bg-surface-2 transition-colors",
                      selected ? "border-(--accent)" : "border-border-soft hover:border-border",
                    )}
                  >
                    <button
                      onClick={() => setViewing(index)}
                      className="block w-full"
                      title={shot.name}
                    >
                      {previews[shot.name] ? (
                        <img
                          src={previews[shot.name]}
                          alt={shot.name}
                          decoding="async"
                          draggable={false}
                          className="aspect-video w-full bg-void object-cover"
                        />
                      ) : (
                        <span className="block aspect-video w-full bg-void" />
                      )}
                    </button>

                    <button
                      onClick={() => toggle(shot.name)}
                      title={selected ? "Deselect" : "Select"}
                      className={cn(
                        "absolute left-2 top-2 grid size-6 place-items-center rounded-md border transition-opacity",
                        selected
                          ? "border-(--accent) bg-(--accent) text-black"
                          : "border-border bg-void/80 text-transparent opacity-0 group-hover:opacity-100",
                      )}
                    >
                      <Check className="size-3.5" />
                    </button>

                    <div className="absolute right-2 top-2 flex gap-1 opacity-0 transition-opacity group-hover:opacity-100">
                      <button
                        onClick={() => void copy(shot)}
                        title="Copy the image"
                        className="grid size-6 place-items-center rounded-md border border-border bg-void/80 text-content-muted transition-colors hover:text-content"
                      >
                        <Copy className="size-3.5" />
                      </button>
                      <button
                        onClick={() => openFile(shot.path)}
                        title="Open outside Basalt"
                        className="grid size-6 place-items-center rounded-md border border-border bg-void/80 text-content-muted transition-colors hover:text-content"
                      >
                        <ExternalLink className="size-3.5" />
                      </button>
                    </div>

                    <div className="flex items-baseline gap-2 px-3 py-2">
                      <span className="min-w-0 flex-1 truncate font-mono text-[11px] text-content-muted">
                        {shot.name}
                      </span>
                      <span className="shrink-0 tabular-nums text-[10px] text-content-faint">
                        {formatBytes(shot.size_bytes)}
                      </span>
                    </div>
                  </div>
                );
              })}
            </div>
          ))}
        </div>
      </div>

      {open &&
        createPortal(
          <div
            className="fixed inset-0 z-50 flex flex-col overflow-hidden bg-black/95"
            onClick={() => setViewing(null)}
          >
            <div
              className="flex shrink-0 items-center gap-3 px-5 py-3"
              onClick={(event) => event.stopPropagation()}
            >
              <div className="min-w-0 flex-1">
                <p className="truncate font-mono text-xs text-content">{open.name}</p>
                <p className="text-[11px] text-content-faint">
                  {formatDateTime(open.modified_ms)} · {formatBytes(open.size_bytes)} · {viewing! + 1} of{" "}
                  {shots.length}
                </p>
              </div>
              <button
                onClick={() => void copy(open)}
                className="inline-flex items-center gap-1.5 rounded-lg border border-border bg-surface-2 px-2.5 py-1.5 text-[11px] font-medium text-content-muted transition-colors hover:bg-surface-3 hover:text-content"
              >
                <Copy className="size-3.5" />
                Copy
              </button>
              <button
                onClick={() => openFile(open.path)}
                className="inline-flex items-center gap-1.5 rounded-lg border border-border bg-surface-2 px-2.5 py-1.5 text-[11px] font-medium text-content-muted transition-colors hover:bg-surface-3 hover:text-content"
              >
                <ExternalLink className="size-3.5" />
                Open
              </button>
              <button
                onClick={() => setConfirming(true)}
                className="inline-flex items-center gap-1.5 rounded-lg border border-danger/40 bg-danger/10 px-2.5 py-1.5 text-[11px] font-semibold text-danger transition-colors hover:bg-danger/20"
              >
                <Trash2 className="size-3.5" />
                Delete
              </button>
              <button
                onClick={() => setViewing(null)}
                className="grid size-8 shrink-0 place-items-center rounded-lg text-content-faint transition-colors hover:bg-white/10 hover:text-content"
              >
                <X className="size-4" />
              </button>
            </div>

            <div className="flex min-h-0 flex-1 items-center gap-2 px-3 pb-5">
              <button
                onClick={(event) => {
                  event.stopPropagation();
                  setViewing((at) => (at == null ? at : Math.max(0, at - 1)));
                }}
                disabled={viewing === 0}
                className="grid size-10 shrink-0 place-items-center rounded-full text-content-muted transition-colors hover:bg-white/10 hover:text-content disabled:opacity-25"
              >
                <ChevronLeft className="size-5" />
              </button>
              <div className="flex min-h-0 min-w-0 flex-1 items-center justify-center">
                <img
                  src={convertFileSrc(open.path)}
                  alt={open.name}
                  draggable={false}
                  onClick={(event) => event.stopPropagation()}
                  className="h-full w-full object-contain"
                />
              </div>
              <button
                onClick={(event) => {
                  event.stopPropagation();
                  setViewing((at) => (at == null ? at : Math.min(shots.length - 1, at + 1)));
                }}
                disabled={viewing === shots.length - 1}
                className="grid size-10 shrink-0 place-items-center rounded-full text-content-muted transition-colors hover:bg-white/10 hover:text-content disabled:opacity-25"
              >
                <ChevronRight className="size-5" />
              </button>
            </div>
          </div>,
          document.body,
        )}

      <ConfirmDialog
        open={confirming}
        nested
        title={
          picked.length > 1 ? `Delete ${picked.length} screenshots?` : "Delete this screenshot?"
        }
        description="The file is removed from disk. This cannot be undone."
        confirmIcon={<Trash2 className="size-3.5" />}
        onConfirm={() => void remove()}
        onCancel={() => setConfirming(false)}
      />
    </div>
  );
}
