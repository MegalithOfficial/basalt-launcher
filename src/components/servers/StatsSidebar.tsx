import { Check, ClipboardCopy, UserRound } from "lucide-react";
import { useMemo, useState } from "react";
import { formatBytes } from "../../lib/format";
import { lanAddress, onlinePlayers, serverAddress } from "../../lib/servers";
import type { ConsoleLine, Server, ServerUsage } from "../../lib/types";
import { useStore } from "../../store";
import { UsageChart } from "./UsageChart";

const EMPTY: ServerUsage[] = [];
const EMPTY_CONSOLE: ConsoleLine[] = [];
const CHART_HEIGHT = 68;

function Address({ label, value }: { label: string; value: string }) {
  const [copied, setCopied] = useState(false);
  return (
    <button
      onClick={() => {
        void navigator.clipboard.writeText(value);
        setCopied(true);
        setTimeout(() => setCopied(false), 1500);
      }}
      title={`Copy ${value}`}
      className="group/address flex w-full items-baseline gap-2 py-0.5 text-left"
    >
      <span className="w-8 shrink-0 font-pixel text-[9px] uppercase tracking-[0.2em] text-content-faint">
        {label}
      </span>
      <span className="min-w-0 flex-1 wrap-break-word font-mono text-[12px] tabular-nums text-content transition-colors group-hover/address:text-(--accent)">
        {value}
      </span>
      {copied ? (
        <Check className="size-3 shrink-0 text-ok" />
      ) : (
        <ClipboardCopy className="size-3 shrink-0 text-content-faint opacity-0 transition-opacity group-hover/address:opacity-100" />
      )}
    </button>
  );
}

function Heading({ label, value }: { label: string; value: React.ReactNode }) {
  return (
    <div className="flex items-baseline justify-between gap-2">
      <span className="font-pixel text-[9px] uppercase tracking-[0.24em] text-content-faint">
        {label}
      </span>
      <span className="font-mono text-[12px] tabular-nums text-content">{value}</span>
    </div>
  );
}

export function StatsSidebar({
  server,
  samples = EMPTY,
  diskBytes,
  live,
  uptime,
  lan,
}: {
  server: Server;
  samples: ServerUsage[] | undefined;
  diskBytes: number | null;
  live: boolean;
  uptime: string;
  lan: string | null;
}) {
  const lines = useStore((s) => s.serverConsole[server.id] ?? EMPTY_CONSOLE);
  const fallback = useStore((s) => s.settings?.server_max_memory_mb ?? 4096);
  const latest = live ? samples[samples.length - 1] : undefined;
  const ceiling = server.max_memory_mb ?? fallback;
  const players = useMemo(() => (live ? onlinePlayers(lines) : []), [lines, live]);
  const lanHost = lanAddress(server, lan);

  return (
    <aside className="flex w-60 shrink-0 flex-col gap-5 border-l border-border-soft px-4 py-3">
      <div>
        <Address label="here" value={serverAddress(server)} />
        {lanHost && <Address label="lan" value={lanHost} />}
        <div className="mt-1 font-mono text-[11px] tabular-nums text-content-faint">
          {live ? `up ${uptime}` : "not running"}
        </div>
      </div>

      <div>
        <Heading
          label="CPU"
          value={latest ? `${latest.cpu_percent.toFixed(0)}%` : "idle"}
        />
        <UsageChart
          samples={samples}
          pick={(sample) => sample.cpu_percent_normalized}
          ceiling={100}
          tone="var(--accent)"
          height={CHART_HEIGHT}
          format={(value) => `${Math.round(value)}%`}
          axisWidth={28}
        />
      </div>

      <div>
        <Heading
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
        />
        <UsageChart
          samples={samples}
          pick={(sample) => sample.memory_mb}
          ceiling={ceiling}
          tone="var(--color-ok)"
          height={CHART_HEIGHT}
          format={(value) => `${Math.round(value)} MB`}
          axisFormat={(value) => `${Math.round(value)}`}
          axisWidth={34}
        />
      </div>

      <Heading
        label="Disk"
        value={diskBytes === null ? "reading" : formatBytes(diskBytes)}
      />

      <div className="flex min-h-0 flex-1 flex-col">
        <Heading label="Players" value={live ? players.length : "idle"} />
        <div className="mt-2 min-h-0 flex-1 overflow-y-auto">
          {players.length === 0 ? (
            <p className="text-[11px] text-content-faint">
              {live ? "Nobody is online." : "Start the server to see who joins."}
            </p>
          ) : (
            players.map((name) => (
              <div
                key={name}
                className="flex items-center gap-2 border-b border-border-soft/40 py-1.5 text-[12px] text-content"
              >
                <UserRound className="size-3.5 shrink-0 text-content-faint" />
                <span className="min-w-0 wrap-break-word">{name}</span>
              </div>
            ))
          )}
        </div>
      </div>
    </aside>
  );
}
