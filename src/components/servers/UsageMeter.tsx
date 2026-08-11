import { cn } from "../../lib/cn";
import type { ServerUsage } from "../../lib/types";

const EMPTY: ServerUsage[] = [];

function Bar({ label, value, hint }: { label: string; value: number; hint: string }) {
  return (
    <div className="min-w-28" title={hint}>
      <div className="flex items-baseline justify-between gap-2">
        <span className="text-[10px] font-semibold uppercase tracking-wider text-content-faint">
          {label}
        </span>
        <span className="font-mono text-[11px] text-content-muted">{hint}</span>
      </div>
      <div className="mt-1 h-1 overflow-hidden rounded-full bg-surface-3">
        <div
          className={cn("h-full rounded-full", value > 0.9 ? "bg-warn" : "bg-(--accent)")}
          style={{ width: `${Math.min(100, Math.max(2, value * 100))}%` }}
        />
      </div>
    </div>
  );
}

export function UsageMeter({
  samples = EMPTY,
  maxMemoryMb,
}: {
  samples: ServerUsage[] | undefined;
  maxMemoryMb: number | null;
}) {
  const latest = samples[samples.length - 1];
  if (!latest) return null;

  const ceiling = maxMemoryMb ?? 0;
  return (
    <div className="flex items-center gap-4">
      <Bar
        label="CPU"
        value={Math.min(1, latest.cpu_percent_normalized / 100)}
        hint={`${latest.cpu_percent.toFixed(0)}% of one core`}
      />
      <Bar
        label="Memory"
        value={ceiling > 0 ? Math.min(1, latest.memory_mb / ceiling) : 0.02}
        hint={ceiling > 0 ? `${latest.memory_mb} / ${ceiling} MB` : `${latest.memory_mb} MB`}
      />
    </div>
  );
}
