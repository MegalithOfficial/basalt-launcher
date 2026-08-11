import { useMemo } from "react";
import { UserRound } from "lucide-react";

import { onlinePlayers } from "../../lib/servers";
import type { ConsoleLine, Server, ServerUsage } from "../../lib/types";
import { useStore } from "../../store";
import { UsageChart } from "./UsageChart";

const EMPTY_CONSOLE: ConsoleLine[] = [];
const CHART_HEIGHT = 110;

function Panel({
  label,
  value,
  children,
}: {
  label: string;
  value: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <div className="min-w-0 flex-1 rounded-xl border border-border-soft bg-surface-2/40 px-4 py-3">
      <div className="flex items-baseline justify-between gap-2">
        <span className="font-pixel text-[9px] uppercase tracking-[0.24em] text-content-faint">
          {label}
        </span>
        <span className="font-mono text-[12px] tabular-nums text-content">{value}</span>
      </div>
      <div className="mt-2">{children}</div>
    </div>
  );
}

export function ConsoleStats({
  server,
  samples,
  live,
}: {
  server: Server;
  samples: ServerUsage[];
  live: boolean;
}) {
  const lines = useStore((s) => s.serverConsole[server.id] ?? EMPTY_CONSOLE);
  const fallback = useStore((s) => s.settings?.server_max_memory_mb ?? 4096);
  const latest = live ? samples[samples.length - 1] : undefined;
  const ceiling = server.max_memory_mb ?? fallback;
  const players = useMemo(() => (live ? onlinePlayers(lines) : []), [lines, live]);

  return (
    <div className="flex shrink-0 gap-3 border-t border-border-soft px-8 py-3">
      <Panel
        label="Memory"
        value={
          latest ? (
            <>
              {latest.memory_mb}
              <span className="text-content-faint"> / {ceiling}</span> MB
            </>
          ) : (
            "idle"
          )
        }
      >
        <UsageChart
          samples={samples}
          pick={(sample) => sample.memory_mb}
          ceiling={ceiling}
          tone="var(--color-ok)"
          height={CHART_HEIGHT}
          format={(value) => `${Math.round(value)} MB`}
          axisFormat={(value) => `${Math.round(value)}`}
          axisWidth={42}
        />
      </Panel>

      <Panel label="CPU" value={latest ? `${latest.cpu_percent.toFixed(0)}%` : "idle"}>
        <UsageChart
          samples={samples}
          pick={(sample) => sample.cpu_percent_normalized}
          ceiling={100}
          tone="var(--accent)"
          height={CHART_HEIGHT}
          format={(value) => `${Math.round(value)}%`}
        />
      </Panel>

      <Panel label="Players" value={live ? players.length : "idle"}>
        <div className="overflow-y-auto" style={{ height: CHART_HEIGHT }}>
          {players.length === 0 ? (
            <p className="text-[11px] text-content-faint">
              {live ? "Nobody is online." : "Start the server to see who joins."}
            </p>
          ) : (
            <div className="flex flex-wrap gap-1.5">
              {players.map((name) => (
                <span
                  key={name}
                  className="inline-flex items-center gap-1.5 rounded-md border border-border-soft bg-surface-2/60 px-2 py-1 text-[11px] text-content"
                >
                  <UserRound className="size-3 shrink-0 text-content-faint" />
                  {name}
                </span>
              ))}
            </div>
          )}
        </div>
      </Panel>
    </div>
  );
}
