import { cn } from "../../lib/cn";
import { formatBytes } from "../../lib/format";
import type { ServerUsage } from "../../lib/types";

const EMPTY: ServerUsage[] = [];

function Stat({
  label,
  value,
  detail,
  fill,
}: {
  label: string;
  value: string;
  detail?: string;
  fill?: number;
}) {
  return (
    <div className="min-w-24">
      <div className="font-pixel text-[9px] uppercase tracking-[0.24em] text-content-faint">
        {label}
      </div>
      <div className="mt-0.5 flex items-baseline gap-1">
        <span className="font-mono text-[13px] tabular-nums text-content">{value}</span>
        {detail && <span className="font-mono text-[10px] text-content-faint">{detail}</span>}
      </div>
      {fill !== undefined && (
        <div className="mt-1 h-0.5 overflow-hidden rounded-full bg-surface-3">
          <div
            className={cn("h-full rounded-full", fill > 0.9 ? "bg-warn" : "bg-(--accent)")}
            style={{ width: `${Math.min(100, Math.max(2, fill * 100))}%` }}
          />
        </div>
      )}
    </div>
  );
}

export function UsageMeter({
  samples = EMPTY,
  maxMemoryMb,
  diskBytes,
  live,
}: {
  samples: ServerUsage[] | undefined;
  maxMemoryMb: number | null;
  diskBytes: number | null;
  live: boolean;
}) {
  const latest = samples[samples.length - 1];
  const ceiling = maxMemoryMb ?? 0;

  return (
    <div className="flex items-start gap-5">
      <Stat
        label="CPU"
        value={live && latest ? `${latest.cpu_percent.toFixed(0)}%` : "idle"}
        detail={live && latest ? "of one core" : undefined}
        fill={live && latest ? Math.min(1, latest.cpu_percent_normalized / 100) : undefined}
      />
      <Stat
        label="Memory"
        value={live && latest ? `${latest.memory_mb} MB` : "idle"}
        detail={ceiling > 0 ? `of ${ceiling}` : undefined}
        fill={live && latest && ceiling > 0 ? Math.min(1, latest.memory_mb / ceiling) : undefined}
      />
      <Stat
        label="Disk"
        value={diskBytes === null ? "reading" : formatBytes(diskBytes)}
        detail={diskBytes === null ? undefined : "on this machine"}
      />
    </div>
  );
}
