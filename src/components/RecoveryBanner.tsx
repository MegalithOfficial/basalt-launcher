import { AnimatePresence, motion } from "motion/react";
import { RotateCcw, TriangleAlert, X } from "lucide-react";

import { useStore } from "../store";

export function RecoveryBanner() {
  const interrupted = useStore((s) => s.interrupted);
  const dismiss = useStore((s) => s.dismissInterrupted);
  const openDiscover = useStore((s) => s.openDiscover);

  const count = interrupted.length;
  if (count === 0) return null;

  const packs = interrupted.filter((op) => op.kind === "ModpackInstall");

  return (
    <AnimatePresence>
      <motion.div
        initial={{ opacity: 0, y: -8 }}
        animate={{ opacity: 1, y: 0 }}
        exit={{ opacity: 0, y: -8 }}
        className="mx-6 mt-3 flex items-start gap-3 rounded-xl border border-warn/30 bg-warn/10 px-4 py-3"
      >
        <TriangleAlert className="mt-0.5 size-4 shrink-0 text-warn" />
        <div className="min-w-0 flex-1">
          <div className="text-sm font-medium text-warn">
            Basalt closed during {count} {count === 1 ? "download" : "downloads"}
          </div>
          <div className="mt-0.5 text-xs text-warn/80">
            Partial files were removed.
            {packs.length > 0 &&
              ` ${packs.length === 1 ? "The pack instance was" : "Those pack instances were"} deleted: ${packs
                .map((p) => p.title)
                .join(", ")}.`}
          </div>
        </div>

        {packs.length > 0 && (
          <button
            onClick={() => {
              dismiss();
              openDiscover("modpacks", null);
            }}
            className="inline-flex shrink-0 items-center gap-1.5 rounded-lg border border-warn/40 bg-warn/10 px-3 py-1.5 text-xs font-semibold text-warn transition-colors hover:bg-warn/20"
          >
            <RotateCcw className="size-3.5" />
            Reinstall
          </button>
        )}

        <button
          onClick={dismiss}
          aria-label="Dismiss"
          className="grid size-7 shrink-0 place-items-center rounded-md text-warn/70 transition-colors hover:bg-warn/15 hover:text-warn"
        >
          <X className="size-3.5" />
        </button>
      </motion.div>
    </AnimatePresence>
  );
}
