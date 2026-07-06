import { AnimatePresence, motion } from "motion/react";
import { Check, Plus } from "lucide-react";

import { cn } from "../lib/cn";
import { loaderLabel } from "../lib/loader";
import { mediaSrc } from "../lib/media";
import { useEscape } from "../lib/useEscape";
import { useStore } from "../store";

const tileGrid: React.CSSProperties = {
  backgroundImage: `
    linear-gradient(to right, rgba(255,255,255,0.05) 1px, transparent 1px),
    linear-gradient(to bottom, rgba(255,255,255,0.05) 1px, transparent 1px)`,
  backgroundSize: "24px 24px",
};

export function InstanceSheet({
  open,
  onClose,
  onCreate,
}: {
  open: boolean;
  onClose: () => void;
  onCreate: () => void;
}) {
  const instances = useStore((s) => s.instances);
  const mediaMap = useStore((s) => s.media);
  const selectedId = useStore((s) => s.selectedInstanceId);
  const selectInstance = useStore((s) => s.selectInstance);

  useEscape(open, onClose);

  return (
    <AnimatePresence>
      {open && (
        <>
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            onClick={onClose}
            className="fixed inset-0 z-40 bg-black/60 backdrop-blur-sm"
          />
          <motion.div
            initial={{ y: "100%" }}
            animate={{ y: 0 }}
            exit={{ y: "100%" }}
            transition={{ type: "spring", stiffness: 380, damping: 40 }}
            className="fixed inset-x-0 bottom-0 z-50 max-h-[70vh] overflow-y-auto rounded-t-3xl border-t border-border bg-surface/95 backdrop-blur-xl"
          >
            <div className="sticky top-0 z-10 flex flex-col items-center bg-gradient-to-b from-surface to-transparent pb-2 pt-3">
              <button
                onClick={onClose}
                aria-label="Close"
                className="h-1.5 w-12 rounded-full bg-border transition-colors hover:bg-content-faint"
              />
              <div className="mt-3 text-[11px] font-semibold uppercase tracking-[0.2em] text-content-faint">
                Instances
              </div>
            </div>

            <div className="grid grid-cols-[repeat(auto-fill,minmax(200px,1fr))] gap-4 p-6 pt-2">
              {instances.map((it) => {
                const media = mediaMap[it.id] ?? null;
                const active = it.id === selectedId;
                return (
                  <button
                    key={it.id}
                    onClick={() => {
                      selectInstance(it.id);
                      onClose();
                    }}
                    className={cn(
                      "group relative aspect-[16/10] overflow-hidden rounded-2xl border text-left transition-all duration-300",
                      active
                        ? "border-transparent ring-2 ring-[var(--accent)] shadow-lg shadow-[var(--accent-glow)]"
                        : "border-border hover:border-content-faint/40",
                    )}
                  >
                    {media ? (
                      <img
                        src={mediaSrc(media)}
                        className={cn(
                          "absolute inset-0 h-full w-full object-cover transition-transform duration-300 group-hover:scale-[1.03]",
                          !media.local && "[image-rendering:pixelated]",
                        )}
                        draggable={false}
                      />
                    ) : (
                      <div className="absolute inset-0 bg-surface-2" style={tileGrid} />
                    )}
                    <div className="absolute inset-x-0 bottom-0 bg-gradient-to-t from-black/85 to-transparent px-4 pb-3 pt-8">
                      <div className="truncate text-sm font-semibold text-white">
                        {it.name}
                      </div>
                      <div className="truncate font-pixel text-[10px] text-white/60">
                        {it.version_id}
                        {it.loader && ` · ${loaderLabel(it)}`}
                      </div>
                    </div>
                    {active && (
                      <span className="absolute right-3 top-3 grid size-6 place-items-center rounded-full bg-[var(--accent)] text-black transition-colors duration-500">
                        <Check className="size-4" />
                      </span>
                    )}
                  </button>
                );
              })}

              <button
                onClick={() => {
                  onClose();
                  onCreate();
                }}
                className="flex aspect-[16/10] flex-col items-center justify-center gap-2 rounded-2xl border border-dashed border-border text-content-faint transition-colors hover:border-[var(--accent)] hover:text-content-muted"
              >
                <Plus className="size-6" />
                <span className="text-sm font-medium">New instance</span>
              </button>
            </div>
          </motion.div>
        </>
      )}
    </AnimatePresence>
  );
}
