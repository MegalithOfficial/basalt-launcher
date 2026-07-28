import { useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { Boxes, Check, ChevronDown, X } from "lucide-react";

import { cn } from "../lib/cn";
import { loaderLabel } from "../lib/loader";
import { useEscape } from "../lib/useEscape";
import type { Instance } from "../lib/types";

function InstanceRow({
  instance,
  selected,
  onClick,
}: {
  instance: Instance;
  selected: boolean;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left transition-colors",
        selected ? "bg-[var(--accent-glow)]" : "hover:bg-surface-2",
      )}
    >
      <div className="grid size-8 shrink-0 place-items-center rounded-lg bg-surface-3 text-content-faint">
        <Boxes className="size-4" />
      </div>
      <div className="min-w-0 flex-1">
        <div className="truncate text-sm font-medium text-content">{instance.name}</div>
        <div className="truncate text-[11px] text-content-faint">
          {instance.version_id} · {loaderLabel(instance)}
        </div>
      </div>
      {selected && <Check className="size-4 shrink-0 text-[var(--accent)]" />}
    </button>
  );
}

export function InstanceTargetPicker({
  instances,
  selected,
  onSelect,
  modalFor,
}: {
  instances: Instance[];
  selected: Instance | null;
  onSelect: (instance: Instance | null) => void;
  modalFor?: string | null;
}) {
  const [open, setOpen] = useState(false);
  const isModal = modalFor != null;
  useEscape(isModal, () => onSelect(null));

  if (isModal) {
    return (
      <AnimatePresence>
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          className="fixed inset-0 z-[60] grid place-items-center bg-black/60 p-6 backdrop-blur-sm"
          onClick={() => onSelect(null)}
        >
          <motion.div
            initial={{ opacity: 0, scale: 0.97, y: 8 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.97, y: 8 }}
            transition={{ duration: 0.18 }}
            onClick={(e) => e.stopPropagation()}
            className="flex max-h-[70vh] w-full max-w-md flex-col overflow-hidden rounded-2xl border border-border bg-surface shadow-2xl"
          >
            <div className="flex items-start justify-between gap-3 border-b border-border-soft px-5 py-4">
              <div className="min-w-0">
                <h2 className="font-display text-base font-semibold text-content">
                  Install to which instance?
                </h2>
                <div className="mt-0.5 truncate text-xs text-content-muted">{modalFor}</div>
              </div>
              <button
                onClick={() => onSelect(null)}
                aria-label="Cancel"
                className="grid size-7 shrink-0 place-items-center rounded-md text-content-faint transition-colors hover:bg-surface-2 hover:text-content"
              >
                <X className="size-4" />
              </button>
            </div>
            <div className="min-h-0 flex-1 overflow-y-auto p-2">
              {instances.length === 0 ? (
                <div className="px-3 py-8 text-center text-sm text-content-faint">
                  Create an instance first.
                </div>
              ) : (
                instances.map((instance) => (
                  <InstanceRow
                    key={instance.id}
                    instance={instance}
                    selected={false}
                    onClick={() => onSelect(instance)}
                  />
                ))
              )}
            </div>
          </motion.div>
        </motion.div>
      </AnimatePresence>
    );
  }

  return (
    <div className="relative">
      <button
        onClick={() => setOpen((v) => !v)}
        className={cn(
          "inline-flex h-8 items-center gap-2 rounded-lg border px-3 text-xs font-medium transition-colors",
          selected
            ? "border-border bg-surface-2 text-content hover:bg-surface-3"
            : "border-dashed border-border text-content-faint hover:text-content",
        )}
      >
        <Boxes className="size-3.5" />
        <span className="max-w-[14rem] truncate">
          {selected ? selected.name : "Choose instance"}
        </span>
        <ChevronDown className={cn("size-3 transition-transform", open && "rotate-180")} />
      </button>

      {open && (
        <>
          <div className="fixed inset-0 z-40" onClick={() => setOpen(false)} />
          <div className="absolute right-0 top-full z-50 mt-1.5 max-h-80 w-72 overflow-y-auto rounded-xl border border-border bg-surface p-1.5 shadow-2xl">
            {instances.length === 0 ? (
              <div className="px-3 py-6 text-center text-xs text-content-faint">
                No instances yet.
              </div>
            ) : (
              instances.map((instance) => (
                <InstanceRow
                  key={instance.id}
                  instance={instance}
                  selected={selected?.id === instance.id}
                  onClick={() => {
                    onSelect(instance);
                    setOpen(false);
                  }}
                />
              ))
            )}
          </div>
        </>
      )}
    </div>
  );
}
