import { Download, Loader2, Play, Terminal } from "lucide-react";

import { cn } from "../lib/cn";
import type { Instance } from "../lib/types";
import { useStore } from "../store";

export function PlayButton({
  instance,
  compact,
  onError,
}: {
  instance: Instance;
  compact?: boolean;
  onError: (message: string | null) => void;
}) {
  const installs = useStore((s) => s.installs);
  const installedIds = useStore((s) => s.installedIds);
  const running = useStore((s) => s.running);
  const installInstance = useStore((s) => s.installInstance);
  const launchInstance = useStore((s) => s.launchInstance);
  const openConsole = useStore((s) => s.openConsole);

  const install = installs[instance.id];
  const installed = installedIds.includes(instance.id);
  const run = Object.values(running).find(
    (r) => r.instance_id === instance.id && r.state === "running",
  );
  const percent =
    install && install.total > 0 ? Math.round((install.completed / install.total) * 100) : 0;

  const base = cn(
    "inline-flex items-center justify-center gap-1.5 rounded-lg text-xs font-semibold transition-all",
    compact ? "h-8 px-3" : "h-9 px-4",
  );

  if (install) {
    return (
      <span className={cn(base, "cursor-default bg-surface-3 text-content-muted")}>
        <Loader2 className="size-3.5 animate-spin" />
        {percent}%
      </span>
    );
  }
  if (run) {
    return (
      <button
        onClick={() => openConsole(run.running_id)}
        className={cn(base, "border border-ok/40 bg-ok/10 text-ok hover:bg-ok/20")}
      >
        <Terminal className="size-3.5" />
        Console
      </button>
    );
  }
  if (!installed) {
    return (
      <button
        onClick={() => installInstance(instance.id)}
        className={cn(base, "border border-border bg-surface-3 text-content hover:bg-border")}
      >
        <Download className="size-3.5" />
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
        "text-black shadow-md shadow-[var(--accent-glow)] [background:linear-gradient(to_bottom,var(--accent),var(--accent-deep))] hover:[background:linear-gradient(to_bottom,var(--accent-bright),var(--accent))]",
      )}
    >
      <Play className="size-3.5 fill-black" />
      Play
    </button>
  );
}
