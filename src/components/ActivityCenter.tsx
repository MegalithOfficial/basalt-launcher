import { useEffect, useRef, useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import {
  ArrowDownToLine,
  Check,
  CircleSlash,
  Loader2,
  Package,
  RotateCw,
  TriangleAlert,
  X,
} from "lucide-react";

import { cn } from "../lib/cn";
import { relativeTime } from "../lib/time";
import type { Task } from "../lib/types";
import {
  aggregateFraction,
  taskFraction,
  useActiveTasks,
  useFinishedTasks,
} from "../lib/useTasks";
import { useStore } from "../store";
import { formatBytes } from "../lib/format";


function Ring({ fraction }: { fraction: number | null }) {
  const r = 9;
  const circumference = 2 * Math.PI * r;
  return (
    <svg viewBox="0 0 24 24" className="absolute inset-0 size-full -rotate-90">
      <circle
        cx="12"
        cy="12"
        r={r}
        fill="none"
        strokeWidth="2"
        className="stroke-current opacity-25"
      />
      {fraction != null && (
        <circle
          cx="12"
          cy="12"
          r={r}
          fill="none"
          strokeWidth="2"
          strokeLinecap="round"
          className="stroke-(--accent) transition-[stroke-dashoffset] duration-300"
          strokeDasharray={circumference}
          strokeDashoffset={circumference * (1 - Math.min(Math.max(fraction, 0), 1))}
        />
      )}
    </svg>
  );
}

function StateIcon({ task }: { task: Task }) {
  if (task.retry_note) return <RotateCw className="size-3.5 animate-spin text-warn" />;
  if (task.state === "succeeded") return <Check className="size-3.5 text-ok" />;
  if (task.state === "failed") return <TriangleAlert className="size-3.5 text-danger" />;
  if (task.state === "cancelled") return <CircleSlash className="size-3.5 text-content-faint" />;
  return <Loader2 className="size-3.5 animate-spin text-(--accent)" />;
}

function Row({ task, onCancel }: { task: Task; onCancel: (id: string) => void }) {
  const fraction = taskFraction(task);
  const active = task.state === "running";
  const cancellable = active && !task.id.startsWith("optimistic:");

  return (
    <div className="flex items-start gap-3 rounded-lg px-2.5 py-2.5 transition-colors hover:bg-surface-2">
      {task.icon_url ? (
        <img
          src={task.icon_url}
          className="size-9 shrink-0 rounded-lg bg-surface-3 object-cover"
          draggable={false}
        />
      ) : (
        <div className="grid size-9 shrink-0 place-items-center rounded-lg bg-surface-3 text-content-faint">
          <Package className="size-4" />
        </div>
      )}

      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="truncate text-[13px] font-medium text-content">{task.title}</span>
          <span className="ml-auto flex shrink-0 items-center gap-1">
            <StateIcon task={task} />
            {cancellable && (
              <button
                onClick={() => onCancel(task.id)}
                aria-label={`Cancel ${task.title}`}
                title="Cancel"
                className="grid size-5 place-items-center rounded text-content-faint transition-colors hover:bg-danger/15 hover:text-danger"
              >
                <X className="size-3" />
              </button>
            )}
          </span>
        </div>

        <div className="truncate text-[11px] text-content-faint">
          {task.error ? (
            <span className="text-danger">{task.error}</span>
          ) : task.retry_note ? (
            <span className="text-warn">{task.retry_note}</span>
          ) : (
            <>
              {!active && task.subtitle && <span>{task.subtitle} · </span>}
              <span className="capitalize">{task.stage}</span>
              {active && task.total > 0 && (
                <span>
                  {" "}
                  · {task.completed}/{task.total}
                </span>
              )}
              {active && task.total_bytes > 0 && (
                <span>
                  {" "}
                  · {formatBytes(task.downloaded_bytes)}/{formatBytes(task.total_bytes)}
                </span>
              )}
              {!active && task.finished_at && (
                <span> · {relativeTime(task.finished_at)}</span>
              )}
              {task.retries > 0 && !task.retry_note && (
                <span className="text-warn">
                  {" "}
                  · retried {task.retries}
                  {task.retries === 1 ? " time" : " times"}
                </span>
              )}
            </>
          )}
        </div>

        {active && (
          <div className="mt-1.5 h-1 overflow-hidden rounded-full bg-surface-3">
            <div
              className={cn(
                "h-full rounded-full",
                task.retry_note ? "bg-warn" : "bg-(--accent)",
                fraction == null
                  ? "w-1/3 animate-pulse"
                  : "transition-[width] duration-300",
              )}
              style={fraction == null ? undefined : { width: `${fraction * 100}%` }}
            />
          </div>
        )}
      </div>
    </div>
  );
}

export function ActivityCenter({ immersive }: { immersive: boolean }) {
  const active = useActiveTasks();
  const finished = useFinishedTasks();
  const clearFinished = useStore((s) => s.clearFinishedTasks);
  const cancelTask = useStore((s) => s.cancelTask);
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  const hasFailure = finished.some((t) => t.state === "failed");
  const fraction = aggregateFraction(active);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    const onEsc = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onEsc);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onEsc);
    };
  }, [open]);

  return (
    <div ref={ref} className="relative">
      <button
        onClick={() => setOpen((v) => !v)}
        aria-label={active.length > 0 ? `${active.length} active downloads` : "Downloads"}
        title={active.length > 0 ? `${active.length} active` : "Downloads"}
        className={cn(
          "relative grid h-8 w-11 place-items-center transition-colors",
          immersive
            ? "text-white/75 drop-shadow-[0_1px_2px_rgba(0,0,0,0.9)] hover:bg-white/15 hover:text-white"
            : "text-content-faint hover:bg-surface-3 hover:text-content",
          active.length > 0 && "text-(--accent)",
        )}
      >
        <span className="relative grid size-7 place-items-center">
          {active.length > 0 && <Ring fraction={fraction} />}
          <ArrowDownToLine className="size-4" />
        </span>

        {active.length > 0 && (
          <span className="absolute right-1.5 top-1 grid min-w-3.5 place-items-center rounded-full bg-(--accent) px-1 text-[9px] font-bold leading-3.5 text-black">
            {active.length}
          </span>
        )}
        {active.length === 0 && hasFailure && (
          <span className="absolute right-2.5 top-1.5 size-1.5 rounded-full bg-danger" />
        )}
      </button>

      <AnimatePresence>
        {open && (
          <motion.div
            initial={{ opacity: 0, y: -6, scale: 0.98 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: -6, scale: 0.98 }}
            transition={{ duration: 0.14 }}
            className="absolute right-0 top-full z-60 mt-1 flex max-h-[70vh] w-96 flex-col overflow-hidden rounded-xl border border-border bg-surface shadow-2xl"
          >
            <div className="flex items-center justify-between border-b border-border-soft px-3 py-2">
              <span className="text-[13px] font-semibold text-content">Downloads</span>
              {finished.length > 0 && (
                <button
                  onClick={() => clearFinished()}
                  className="rounded-md px-1.5 py-0.5 text-[11px] text-content-faint transition-colors hover:bg-surface-2 hover:text-content"
                >
                  Clear finished
                </button>
              )}
            </div>

            <div className="min-h-0 flex-1 overflow-y-auto p-1.5">
              {active.length === 0 && finished.length === 0 && (
                <div className="px-3 py-8 text-center text-xs text-content-faint">
                  Nothing downloading.
                  <div className="mt-1 text-[11px] opacity-70">
                    Installs and updates show up here.
                  </div>
                </div>
              )}

              {active.length > 0 && (
                <>
                  <div className="px-2 pb-1 pt-1 text-[10px] font-semibold uppercase tracking-wider text-content-faint">
                    Active
                  </div>
                  {active.map((task) => (
                    <Row key={task.id} task={task} onCancel={cancelTask} />
                  ))}
                </>
              )}

              {finished.length > 0 && (
                <>
                  <div className="px-2 pb-1 pt-2 text-[10px] font-semibold uppercase tracking-wider text-content-faint">
                    Recent
                  </div>
                  {finished.slice(0, 20).map((task) => (
                    <Row key={task.id} task={task} onCancel={cancelTask} />
                  ))}
                </>
              )}
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
