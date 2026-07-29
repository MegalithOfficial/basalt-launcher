import { cn } from "../lib/cn";
import { useUptime } from "../lib/useUptime";
import { useStore } from "../store";

export function RunningPill({ immersive }: { immersive?: boolean }) {
  const running = useStore((s) => s.running);
  const instances = useStore((s) => s.instances);
  const openConsole = useStore((s) => s.openConsole);

  const live = Object.values(running)
    .filter((r) => r.state === "running")
    .sort((a, b) => a.started_at - b.started_at);

  const first = live[0];
  const name = instances.find((i) => i.id === first?.instance_id)?.name ?? "Instance";
  const uptime = useUptime(first?.started_at ?? 0, !!first);

  if (!first) return null;

  return (
    <button
      onClick={() => openConsole(first.running_id)}
      title={`${name} is running. Open its logs.`}
      className={cn(
        "mr-1 flex h-6 max-w-56 items-center gap-2 rounded-full border px-2.5 text-[11px] font-medium transition-colors",
        immersive
          ? "border-white/20 bg-black/50 text-white/85 backdrop-blur hover:bg-black/70"
          : "border-ok/30 bg-ok/10 text-content-muted hover:bg-ok/20 hover:text-content",
      )}
    >
      <span className="relative flex size-1.5 shrink-0">
        <span className="absolute inline-flex size-full animate-ping rounded-full bg-ok opacity-75" />
        <span className="relative inline-flex size-1.5 rounded-full bg-ok" />
      </span>
      <span className="min-w-0 truncate">
        {live.length > 1 ? `${live.length} running` : name}
      </span>
      <span className="shrink-0 tabular-nums opacity-70">{uptime}</span>
    </button>
  );
}
