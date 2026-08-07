import { useEffect, useState } from "react";

import { cn } from "../lib/cn";

function clock(seconds: number): string {
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const rest = seconds % 60;
  const pad = (value: number) => String(value).padStart(2, "0");
  return hours > 0 ? `${hours}:${pad(minutes)}:${pad(rest)}` : `${pad(minutes)}:${pad(rest)}`;
}

export function DiscordPreview({
  enabled,
  showVersion,
  showStreak,
  showLogo,
}: {
  enabled: boolean;
  showVersion: boolean;
  showStreak: boolean;
  showLogo: boolean;
}) {
  const [elapsed, setElapsed] = useState(2534);

  useEffect(() => {
    if (!enabled) return;
    const timer = setInterval(() => setElapsed((value) => value + 1), 1000);
    return () => clearInterval(timer);
  }, [enabled]);

  return (
    <div
      className={cn(
        "w-full max-w-xs shrink-0 rounded-xl border border-border-soft bg-void p-3.5 transition-opacity",
        !enabled && "opacity-40",
      )}
    >
      <div className="font-pixel text-[9px] uppercase tracking-[0.28em] text-content-faint">
        Playing
      </div>
      <div className="mt-2.5 flex gap-3">
        <div
          className={cn(
            "grid size-14 shrink-0 place-items-center overflow-hidden rounded-lg border border-border-soft",
            showLogo ? "bg-surface-3" : "bg-surface-2",
          )}
        >
          {showLogo ? (
            <img src="/logo.png" alt="" draggable={false} className="size-9 object-contain" />
          ) : (
            <span className="font-pixel text-[8px] tracking-wider text-content-faint">B</span>
          )}
        </div>
        <div className="min-w-0 flex-1 leading-tight">
          <div className="text-[13px] font-semibold text-content">Basalt</div>
          <div className="mt-0.5 truncate text-[12px] text-content-muted">
            ATM10 To the Sky
          </div>
          {showVersion && (
            <div className="mt-0.5 font-pixel text-[9px] tracking-wider text-content-faint">
              1.21.1 · neoforge
            </div>
          )}
          <div className="mt-1.5 flex flex-wrap items-center gap-x-2.5 gap-y-0.5 text-[11px] text-content-faint">
            <span className="tabular-nums text-(--accent)">{clock(elapsed)}</span>
            {showStreak && <span>4 day streak</span>}
          </div>
        </div>
      </div>
    </div>
  );
}
