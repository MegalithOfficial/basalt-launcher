import { Download, Loader2, Play } from "lucide-react";

import { cn } from "../lib/cn";
import type { Instance } from "../lib/types";
import { useUptime } from "../lib/useUptime";
import { taskFraction, useInstanceTask } from "../lib/useTasks";
import { useStore } from "../store";

export function PlayButton({
  instance,
  compact,
  hero,
  onError,
}: {
  instance: Instance;
  compact?: boolean;
  hero?: boolean;
  onError: (message: string | null) => void;
}) {
  const installedIds = useStore((s) => s.installedIds);
  const running = useStore((s) => s.running);
  const installInstance = useStore((s) => s.installInstance);
  const launchInstance = useStore((s) => s.launchInstance);
  const openConsole = useStore((s) => s.openConsole);
  const launching = useStore((s) => s.launching);

  const install = useInstanceTask(instance.id);
  const installed = installedIds.includes(instance.id);
  const run = Object.values(running).find(
    (r) => r.instance_id === instance.id && r.state === "running",
  );
  const fraction = install ? taskFraction(install) : null;
  const percent = fraction == null ? 0 : Math.round(fraction * 100);
  const starting = launching.includes(instance.id);
  const uptime = useUptime(run?.started_at ?? 0, !!run);

  const base = cn(
    "inline-flex items-center justify-center gap-1.5 font-semibold transition-all",
    hero ? "h-11 rounded-xl px-6 text-sm" : "rounded-lg text-xs",
    hero ? "min-w-28" : compact ? "h-8 px-3" : "h-9 px-4",
  );
  const glyph = hero ? "size-4" : "size-3.5";

  if (install) {
    return (
      <span className={cn(base, "cursor-default bg-surface-3 text-content-muted")}>
        <Loader2 className={cn(glyph, "animate-spin")} />
        {percent}%
      </span>
    );
  }
  if (starting) {
    return (
      <span className={cn(base, "cursor-default bg-surface-3 text-content-muted")}>
        <Loader2 className={cn(glyph, "animate-spin")} />
        Starting
      </span>
    );
  }
  if (run) {
    return (
      <button
        onClick={() => openConsole(run.running_id)}
        title="Running. Open its logs."
        className={cn(base, "border border-ok/40 bg-ok/10 text-ok hover:bg-ok/20")}
      >
        <span className="relative flex size-1.5 shrink-0">
          <span className="absolute inline-flex size-full animate-ping rounded-full bg-ok opacity-75" />
          <span className="relative inline-flex size-1.5 rounded-full bg-ok" />
        </span>
        {compact ? uptime : `Running ${uptime}`}
      </button>
    );
  }
  if (!installed) {
    return (
      <button
        onClick={() => installInstance(instance.id)}
        className={cn(base, "border border-border bg-surface-3 text-content hover:bg-border")}
      >
        <Download className={glyph} />
        Install
      </button>
    );
  }
  return (
    <button
      onClick={async () => {
        onError(null);
        try {
          await launchInstance(instance.id);
        } catch (e) {
          onError(String(e));
        }
      }}
      className={cn(
        base,
        "text-black shadow-md shadow-(color:--accent-glow) [background:linear-gradient(to_bottom,var(--accent),var(--accent-deep))] hover:[background:linear-gradient(to_bottom,var(--accent-bright),var(--accent))]",
      )}
    >
      <Play className={cn(glyph, "fill-black")} />
      Play
    </button>
  );
}
