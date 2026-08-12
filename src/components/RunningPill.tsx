import { cn } from "../lib/cn";
import { useUptime } from "../lib/useUptime";
import { useStore } from "../store";

interface LiveTarget {
  id: string;
  name: string;
  startedAt: number;
}

function LivePill({
  first,
  count,
  label,
  server,
  immersive,
  onOpen,
}: {
  first: LiveTarget | undefined;
  count: number;
  label: string;
  server?: boolean;
  immersive?: boolean;
  onOpen: (id: string) => void;
}) {
  const uptime = useUptime(first?.startedAt ?? 0, !!first);
  if (!first) return null;

  return (
    <button
      onClick={() => onOpen(first.id)}
      title={`${first.name} is running. ${server ? "Open the server." : "Open its logs."}`}
      className={cn(
        "mr-1 flex h-6 max-w-56 items-center gap-2 rounded-full border px-2.5 text-[11px] font-medium transition-colors",
        server
          ? immersive
            ? "border-lava/40 bg-black/50 text-white/85 backdrop-blur hover:bg-lava/15"
            : "border-lava/40 bg-lava/10 text-lava hover:bg-lava/20"
          : immersive
            ? "border-white/20 bg-black/50 text-white/85 backdrop-blur hover:bg-black/70"
            : "border-ok/30 bg-ok/10 text-content-muted hover:bg-ok/20 hover:text-content",
      )}
    >
      <span className="relative flex size-1.5 shrink-0">
        <span
          className={cn(
            "absolute inline-flex size-full animate-ping rounded-full opacity-75",
            server ? "bg-lava" : "bg-ok",
          )}
        />
        <span
          className={cn(
            "relative inline-flex size-1.5 rounded-full",
            server ? "bg-lava" : "bg-ok",
          )}
        />
      </span>
      <span className="min-w-0 truncate">{count > 1 ? `${count} ${label}` : first.name}</span>
      <span className="shrink-0 tabular-nums opacity-70">{uptime}</span>
    </button>
  );
}

export function RunningPill({ immersive }: { immersive?: boolean }) {
  const running = useStore((s) => s.running);
  const instances = useStore((s) => s.instances);
  const serverRunning = useStore((s) => s.serverRunning);
  const servers = useStore((s) => s.servers);
  const openConsole = useStore((s) => s.openConsole);
  const openServer = useStore((s) => s.openServer);

  const liveInstances = Object.values(running)
    .filter((run) => run.state === "running")
    .map((run) => ({
      id: run.running_id,
      name: instances.find((instance) => instance.id === run.instance_id)?.name ?? "Instance",
      startedAt: run.started_at,
    }))
    .sort((a, b) => a.startedAt - b.startedAt);
  const liveServers = Object.values(serverRunning)
    .filter((run) => run.state === "running")
    .map((run) => ({
      id: run.server_id,
      name: servers.find((server) => server.id === run.server_id)?.name ?? "Server",
      startedAt: run.started_at,
    }))
    .sort((a, b) => a.startedAt - b.startedAt);

  return (
    <>
      <LivePill
        first={liveInstances[0]}
        count={liveInstances.length}
        label="running"
        immersive={immersive}
        onOpen={openConsole}
      />
      <LivePill
        first={liveServers[0]}
        count={liveServers.length}
        label="servers"
        server
        immersive={immersive}
        onOpen={openServer}
      />
    </>
  );
}
