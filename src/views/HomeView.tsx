import { useState } from "react";
import { motion } from "motion/react";
import { Boxes, Check, Play, Plus, Sparkles } from "lucide-react";

import { cn } from "../lib/cn";
import { useStore } from "../store";

// Faint blueprint grid (minor + major lines) with a lava glow — the hero backdrop.
const gridStyle: React.CSSProperties = {
  backgroundImage: `
    linear-gradient(to right, rgba(255,255,255,0.035) 1px, transparent 1px),
    linear-gradient(to bottom, rgba(255,255,255,0.035) 1px, transparent 1px),
    linear-gradient(to right, rgba(255,255,255,0.05) 1px, transparent 1px),
    linear-gradient(to bottom, rgba(255,255,255,0.05) 1px, transparent 1px)`,
  backgroundSize: "32px 32px, 32px 32px, 128px 128px, 128px 128px",
};

export function HomeView() {
  const setView = useStore((s) => s.setView);
  const instances = useStore((s) => s.instances);

  const [selectedId, setSelectedId] = useState<string | null>(instances[0]?.id ?? null);
  const selected = instances.find((i) => i.id === selectedId) ?? instances[0];
  const hasInstance = !!selected;

  // Account wiring lands in M3; for now nobody is signed in.
  const account = null as { name: string } | null;

  const playLabel = !hasInstance ? "CREATE INSTANCE" : !account ? "SIGN IN TO PLAY" : "PLAY";
  const onPlay = () => {
    if (!hasInstance) setView("instances");
    else if (!account) setView("accounts");
    // else: launch — arrives in Milestone 4.
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-4 p-5">
      <motion.div
        initial={{ opacity: 0, y: 10 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.3, ease: "easeOut" }}
        className="relative flex min-h-0 flex-1 flex-col overflow-hidden rounded-2xl border border-border bg-surface"
      >
        <div className="absolute inset-0" style={gridStyle} />
        <div className="pointer-events-none absolute -left-24 -top-24 size-72 rounded-full bg-lava/10 blur-3xl" />
        <div className="pointer-events-none absolute inset-x-0 bottom-0 h-32 bg-gradient-to-t from-base/70 to-transparent" />

        {hasInstance ? (
          <div className="relative flex min-h-0 flex-1 flex-col">
            <div className="flex items-center justify-between px-6 pb-3 pt-5">
              <div className="flex items-center gap-2 text-[11px] font-semibold uppercase tracking-[0.18em] text-content-faint">
                <Boxes className="size-3.5" />
                Your instances
              </div>
              <span className="text-xs text-content-faint">
                {instances.length} total
              </span>
            </div>

            <div className="grid min-h-0 flex-1 auto-rows-min grid-cols-[repeat(auto-fill,minmax(180px,1fr))] content-start gap-3 overflow-y-auto px-6 pb-6">
              {instances.map((it) => {
                const active = it.id === selected.id;
                return (
                  <button
                    key={it.id}
                    onClick={() => setSelectedId(it.id)}
                    className={cn(
                      "group relative overflow-hidden rounded-xl border p-4 text-left transition-all",
                      active
                        ? "border-lava/60 bg-lava/10 ring-1 ring-lava/40"
                        : "border-border bg-base/40 backdrop-blur hover:border-border hover:bg-surface-2/60",
                    )}
                  >
                    {active && (
                      <span className="absolute right-3 top-3 grid size-5 place-items-center rounded-full bg-lava text-black">
                        <Check className="size-3.5" />
                      </span>
                    )}
                    <div className="grid size-9 place-items-center rounded-lg bg-surface-3 text-content-muted">
                      <Boxes className="size-[18px]" />
                    </div>
                    <div className="mt-3 truncate font-display font-semibold text-content">
                      {it.name}
                    </div>
                    <div className="mt-0.5 truncate text-xs text-content-muted">
                      {it.version_id}
                    </div>
                  </button>
                );
              })}

              <button
                onClick={() => setView("instances")}
                className="flex min-h-[116px] flex-col items-center justify-center gap-2 rounded-xl border border-dashed border-border bg-transparent text-content-faint transition-colors hover:border-lava/50 hover:text-content-muted"
              >
                <Plus className="size-5" />
                <span className="text-xs font-medium">New instance</span>
              </button>
            </div>
          </div>
        ) : (
          <div className="relative flex min-h-0 flex-1 flex-col items-start justify-center p-8">
            <div className="mb-4 inline-flex items-center gap-1.5 rounded-full border border-lava/30 bg-lava/10 px-2.5 py-1 text-[11px] font-semibold text-ember">
              <Sparkles className="size-3" />
              Get started
            </div>
            <h1 className="font-pixel text-4xl leading-tight text-content drop-shadow">
              No instance yet
            </h1>
            <p className="mt-4 max-w-md text-sm leading-relaxed text-content-muted">
              Create an instance to pick a Minecraft version and start playing. Mods and modpacks
              come later.
            </p>
          </div>
        )}
      </motion.div>

      <button
        onClick={onPlay}
        className="group flex h-14 shrink-0 items-center justify-center gap-2.5 rounded-2xl bg-gradient-to-b from-lava to-lava-deep font-pixel text-base tracking-wider text-black shadow-xl shadow-lava/25 transition-all hover:from-lava-bright hover:to-lava active:scale-[0.99]"
      >
        {hasInstance && account ? (
          <Play className="size-5 fill-black" />
        ) : (
          <Plus className="size-5" />
        )}
        {playLabel}
      </button>
    </div>
  );
}
