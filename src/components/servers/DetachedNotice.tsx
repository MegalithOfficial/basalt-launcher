import { Loader2, RotateCw, Square } from "lucide-react";

import { useStore } from "../../store";

export function DetachedNotice({ serverId, busy }: { serverId: string; busy?: boolean }) {
  const restartServer = useStore((s) => s.restartServer);
  const forceStopServer = useStore((s) => s.forceStopServer);

  return (
    <div className="flex flex-wrap items-center gap-3 border-b border-warn/30 bg-warn/8 px-6 py-2.5">
      <span className="min-w-0 flex-1 text-[12px] text-warn">
        This server is running from before Basalt restarted. Its console is gone, so commands
        cannot reach it.
      </span>
      <button
        onClick={() => void restartServer(serverId)}
        disabled={busy}
        className="inline-flex items-center gap-1.5 rounded-lg border border-border bg-surface-2 px-3 py-1.5 text-[12px] font-medium text-content transition-colors hover:bg-surface-3 disabled:opacity-50"
      >
        {busy ? <Loader2 className="size-3.5 animate-spin" /> : <RotateCw className="size-3.5" />}
        Restart it
      </button>
      <button
        onClick={() => void forceStopServer(serverId)}
        disabled={busy}
        className="inline-flex items-center gap-1.5 rounded-lg border border-border bg-surface-2 px-3 py-1.5 text-[12px] font-medium text-content-muted transition-colors hover:bg-danger/15 hover:text-danger disabled:opacity-50"
      >
        <Square className="size-3.5" />
        Force stop
      </button>
    </div>
  );
}
