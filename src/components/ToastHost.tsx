import { useEffect } from "react";
import { AnimatePresence, motion } from "motion/react";
import { Check, TriangleAlert, X } from "lucide-react";

import { cn } from "../lib/cn";
import type { Toast } from "../lib/types";
import { useStore } from "../store";

const DISMISS_AFTER: Record<Toast["tone"], number> = {
  success: 4500,
  error: 11000,
};

function ToastCard({ toast, onDismiss }: { toast: Toast; onDismiss: () => void }) {
  useEffect(() => {
    const timer = setTimeout(onDismiss, DISMISS_AFTER[toast.tone]);
    return () => clearTimeout(timer);
  }, [toast.id]);

  const success = toast.tone === "success";

  return (
    <motion.div
      layout
      initial={{ opacity: 0, x: -24, scale: 0.97 }}
      animate={{ opacity: 1, x: 0, scale: 1 }}
      exit={{ opacity: 0, x: -24, scale: 0.97 }}
      transition={{ duration: 0.18 }}
      className={cn(
        "pointer-events-auto flex w-80 items-start gap-3 rounded-xl border bg-surface px-3.5 py-3 shadow-2xl",
        success ? "border-ok/40" : "border-danger/40",
      )}
    >
      <span
        className={cn(
          "mt-0.5 grid size-5 shrink-0 place-items-center rounded-full",
          success ? "bg-ok/20 text-ok" : "bg-danger/20 text-danger",
        )}
      >
        {success ? <Check className="size-3" /> : <TriangleAlert className="size-3" />}
      </span>

      <div className="min-w-0 flex-1">
        <div
          className={cn(
            "truncate text-[13px] font-medium",
            success ? "text-ok" : "text-danger",
          )}
        >
          {toast.title}
        </div>
        {toast.message && (
          <div
            className={cn(
              "mt-0.5 line-clamp-3 text-[11px]",
              success ? "text-ok/75" : "text-danger/80",
            )}
          >
            {toast.message}
          </div>
        )}
      </div>

      <button
        onClick={onDismiss}
        aria-label="Dismiss"
        className={cn(
          "grid size-6 shrink-0 place-items-center rounded-md transition-colors",
          success
            ? "text-ok/60 hover:bg-ok/15 hover:text-ok"
            : "text-danger/60 hover:bg-danger/15 hover:text-danger",
        )}
      >
        <X className="size-3.5" />
      </button>
    </motion.div>
  );
}

export function ToastHost() {
  const toasts = useStore((s) => s.toasts);
  const dismiss = useStore((s) => s.dismissToast);

  return (
    <div className="pointer-events-none fixed bottom-4 left-[84px] z-[80] flex flex-col items-start gap-2">
      <AnimatePresence initial={false}>
        {toasts.map((toast) => (
          <ToastCard key={toast.id} toast={toast} onDismiss={() => dismiss(toast.id)} />
        ))}
      </AnimatePresence>
    </div>
  );
}
