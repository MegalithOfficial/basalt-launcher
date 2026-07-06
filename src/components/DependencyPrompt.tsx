import { AnimatePresence, motion } from "motion/react";
import { Download, Loader2, Package, X } from "lucide-react";

import { useEscape } from "../lib/useEscape";
import type { SearchResult } from "../lib/types";

export interface DependencyPromptData {
  title: string;
  deps: SearchResult[];
}

export function DependencyPrompt({
  prompt,
  busy,
  onInstallAll,
  onSkip,
  onCancel,
}: {
  prompt: DependencyPromptData | null;
  busy: boolean;
  onInstallAll: () => void;
  onSkip: () => void;
  onCancel: () => void;
}) {
  useEscape(!!prompt && !busy, onCancel);

  return (
    <AnimatePresence>
      {prompt && (
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
            className="w-full max-w-md overflow-hidden rounded-2xl border border-border bg-surface shadow-2xl"
          >
            <div className="flex items-center justify-between border-b border-border-soft px-5 py-4">
              <h2 className="font-display text-base font-semibold text-content">
                {prompt.title} needs {prompt.deps.length}{" "}
                {prompt.deps.length === 1 ? "dependency" : "dependencies"}
              </h2>
              <button
                onClick={onCancel}
                disabled={busy}
                className="grid size-7 place-items-center rounded-md text-content-faint hover:bg-surface-2 hover:text-content"
              >
                <X className="size-4" />
              </button>
            </div>

            <div className="max-h-64 overflow-y-auto px-5 py-3">
              {prompt.deps.map((dep) => (
                <div key={dep.id} className="flex items-center gap-3 py-2">
                  {dep.icon_url ? (
                    <img
                      src={dep.icon_url}
                      className="size-9 shrink-0 rounded-lg bg-surface-2 object-cover"
                      draggable={false}
                    />
                  ) : (
                    <div className="grid size-9 shrink-0 place-items-center rounded-lg bg-surface-2 text-content-faint">
                      <Package className="size-4" />
                    </div>
                  )}
                  <div className="min-w-0">
                    <div className="truncate text-sm font-medium text-content">{dep.title}</div>
                    <div className="truncate text-xs text-content-muted">{dep.description}</div>
                  </div>
                </div>
              ))}
            </div>

            <div className="flex items-center justify-end gap-2 border-t border-border-soft px-5 py-4">
              <button
                onClick={onCancel}
                disabled={busy}
                className="rounded-lg px-3 py-2 text-sm font-medium text-content-muted hover:text-content"
              >
                Cancel
              </button>
              <button
                onClick={onSkip}
                disabled={busy}
                className="rounded-lg border border-border bg-surface-2 px-3 py-2 text-sm font-medium text-content hover:bg-surface-3"
              >
                Just this mod
              </button>
              <button
                onClick={onInstallAll}
                disabled={busy}
                className="inline-flex items-center gap-2 rounded-lg px-4 py-2 text-sm font-semibold text-black shadow-lg shadow-[var(--accent-glow)] transition-all [background:linear-gradient(to_bottom,var(--accent),var(--accent-deep))] hover:[background:linear-gradient(to_bottom,var(--accent-bright),var(--accent))]"
              >
                {busy ? <Loader2 className="size-4 animate-spin" /> : <Download className="size-4" />}
                Install all
              </button>
            </div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
