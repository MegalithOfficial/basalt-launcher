import { AnimatePresence, motion } from "motion/react";
import {
  ArrowRight,
  Check,
  Download,
  Loader2,
  Package,
  TriangleAlert,
  X,
} from "lucide-react";

import { cn } from "../lib/cn";
import { useEscape } from "../lib/useEscape";
import type { InstallPlan, PlannedFile, Task } from "../lib/types";
import { taskFraction } from "../lib/useTasks";

function formatBytes(bytes: number): string {
  if (!bytes) return "";
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function Thumb({ url, size = "size-8" }: { url: string | null; size?: string }) {
  return url ? (
    <img
      src={url}
      className={cn(size, "shrink-0 rounded-lg bg-surface-2 object-cover")}
      draggable={false}
    />
  ) : (
    <div className={cn(size, "grid shrink-0 place-items-center rounded-lg bg-surface-2 text-content-faint")}>
      <Package className="size-4" />
    </div>
  );
}

function Row({ file, tone }: { file: PlannedFile; tone: "primary" | "dependency" }) {
  return (
    <div className="flex items-center gap-3 py-1.5">
      <Thumb url={file.icon_url} />
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="truncate text-sm font-medium text-content">{file.title}</span>
          {tone === "dependency" && (
            <span className="shrink-0 rounded bg-surface-3 px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-wide text-content-faint">
              dependency
            </span>
          )}
        </div>
        <div className="flex items-center gap-1.5 truncate text-[11px] text-content-faint">
          {file.replaces ? (
            <>
              <span className="truncate line-through opacity-70">{file.replaces}</span>
              <ArrowRight className="size-2.5 shrink-0" />
            </>
          ) : null}
          <span className="truncate">{file.file_name}</span>
          {file.size != null && <span className="shrink-0">· {formatBytes(file.size)}</span>}
        </div>
      </div>
    </div>
  );
}

export function InstallPlanPrompt({
  plan,
  busy,
  progress,
  onConfirm,
  onSkipDependencies,
  onCancel,
}: {
  plan: InstallPlan | null;
  busy: boolean;
  progress: Task | null;
  onConfirm: () => void;
  onSkipDependencies: () => void;
  onCancel: () => void;
}) {
  useEscape(!!plan && !busy, onCancel);

  const deps = plan?.dependencies ?? [];
  const skipped = plan?.skipped ?? [];
  const conflicts = plan?.conflicts ?? [];
  const present = plan?.already_present ?? [];
  const total = (plan?.primary ? 1 : 0) + deps.length;

  return (
    <AnimatePresence>
      {plan && (
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          className="fixed inset-0 z-[60] grid place-items-center bg-black/60 p-6 backdrop-blur-sm"
          onClick={busy ? undefined : onCancel}
        >
          <motion.div
            initial={{ opacity: 0, scale: 0.97, y: 8 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.97, y: 8 }}
            transition={{ duration: 0.18 }}
            onClick={(e) => e.stopPropagation()}
            className="flex max-h-[80vh] w-full max-w-lg flex-col overflow-hidden rounded-2xl border border-border bg-surface shadow-2xl"
          >
            <div className="flex items-start justify-between gap-3 border-b border-border-soft px-5 py-4">
              <div className="min-w-0">
                <h2 className="truncate font-display text-base font-semibold text-content">
                  Install {plan.primary?.title ?? "content"}
                </h2>
                <div className="mt-0.5 text-xs text-content-muted">
                  {total} {total === 1 ? "file" : "files"}
                  {plan.total_bytes > 0 && ` · ${formatBytes(plan.total_bytes)}`}
                  {deps.length > 0 &&
                    ` · ${deps.length} ${deps.length === 1 ? "dependency" : "dependencies"}`}
                </div>
              </div>
              <button
                onClick={onCancel}
                disabled={busy}
                aria-label="Cancel"
                className="grid size-7 shrink-0 place-items-center rounded-md text-content-faint transition-colors hover:bg-surface-2 hover:text-content disabled:opacity-40"
              >
                <X className="size-4" />
              </button>
            </div>

            <div className="min-h-0 flex-1 overflow-y-auto px-5 py-3">
              {plan.primary && <Row file={plan.primary} tone="primary" />}

              {deps.length > 0 && (
                <>
                  <div className="mb-1 mt-3 text-[10px] font-semibold uppercase tracking-wider text-content-faint">
                    Also installing
                  </div>
                  {deps.map((file) => (
                    <Row key={file.project_id} file={file} tone="dependency" />
                  ))}
                </>
              )}

              {present.length > 0 && (
                <div className="mt-3 rounded-lg bg-surface-2/60 px-3 py-2">
                  <div className="mb-1 flex items-center gap-1.5 text-[10px] font-semibold uppercase tracking-wider text-content-faint">
                    <Check className="size-3 text-ok" />
                    Already installed
                  </div>
                  <div className="text-xs text-content-muted">
                    {present.map((p) => p.title).join(", ")}
                  </div>
                </div>
              )}

              {conflicts.length > 0 && (
                <div className="mt-3 rounded-lg border border-warn/30 bg-warn/10 px-3 py-2">
                  <div className="mb-1 flex items-center gap-1.5 text-[10px] font-semibold uppercase tracking-wider text-warn">
                    <TriangleAlert className="size-3" />
                    Conflicts
                  </div>
                  {conflicts.map((c) => (
                    <div key={c.project_id} className="text-xs text-warn">
                      {c.title}: {c.reason}
                    </div>
                  ))}
                </div>
              )}

              {skipped.length > 0 && (
                <div className="mt-3 rounded-lg border border-warn/30 bg-warn/10 px-3 py-2">
                  <div className="mb-1 flex items-center gap-1.5 text-[10px] font-semibold uppercase tracking-wider text-warn">
                    <TriangleAlert className="size-3" />
                    Could not resolve
                  </div>
                  {skipped.map((s) => (
                    <div key={s.project_id} className="py-0.5 text-xs text-warn">
                      <span className="font-medium">{s.title}</span>
                      <span className="opacity-80"> · {s.reason}</span>
                    </div>
                  ))}
                </div>
              )}
            </div>

            {busy && progress && (
              <div className="border-t border-border-soft px-5 py-2.5">
                <div className="mb-1.5 flex items-center justify-between text-[11px] text-content-muted">
                  <span className="truncate capitalize">{progress.stage}</span>
                  {progress.total > 0 && (
                    <span className="shrink-0 tabular-nums">
                      {progress.completed}/{progress.total}
                    </span>
                  )}
                </div>
                <div className="h-1 overflow-hidden rounded-full bg-surface-3">
                  <div
                    className={cn(
                      "h-full rounded-full bg-[var(--accent)]",
                      taskFraction(progress) == null
                        ? "w-1/3 animate-pulse"
                        : "transition-[width] duration-300",
                    )}
                    style={
                      taskFraction(progress) == null
                        ? undefined
                        : { width: `${(taskFraction(progress) ?? 0) * 100}%` }
                    }
                  />
                </div>
              </div>
            )}

            <div className="flex items-center justify-end gap-2 border-t border-border-soft px-5 py-4">
              <button
                onClick={onCancel}
                disabled={busy}
                className="rounded-lg px-3 py-2 text-sm font-medium text-content-muted transition-colors hover:text-content disabled:opacity-40"
              >
                Cancel
              </button>
              {deps.length > 0 && (
                <button
                  onClick={onSkipDependencies}
                  disabled={busy}
                  className="rounded-lg border border-border bg-surface-2 px-3 py-2 text-sm font-medium text-content transition-colors hover:bg-surface-3 disabled:opacity-40"
                >
                  Skip dependencies
                </button>
              )}
              <button
                onClick={onConfirm}
                disabled={busy}
                className="inline-flex items-center gap-2 rounded-lg px-4 py-2 text-sm font-semibold text-black shadow-lg shadow-[var(--accent-glow)] transition-all [background:linear-gradient(to_bottom,var(--accent),var(--accent-deep))] hover:[background:linear-gradient(to_bottom,var(--accent-bright),var(--accent))] disabled:opacity-60"
              >
                {busy ? (
                  <Loader2 className="size-4 animate-spin" />
                ) : (
                  <Download className="size-4" />
                )}
                {deps.length > 0 ? `Install ${total} files` : "Install"}
              </button>
            </div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
