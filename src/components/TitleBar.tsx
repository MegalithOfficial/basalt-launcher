import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, Square, X } from "lucide-react";

import { cn } from "../lib/cn";
import { ActivityCenter } from "./ActivityCenter";
import { RunningPill } from "./RunningPill";

const win = getCurrentWindow();

function Control({
  onClick,
  label,
  danger,
  immersive,
  children,
}: {
  onClick: () => void;
  label: string;
  danger?: boolean;
  immersive?: boolean;
  children: React.ReactNode;
}) {
  return (
    <button
      aria-label={label}
      onClick={onClick}
      className={cn(
        "grid h-8 w-11 place-items-center transition-colors",
        immersive
          ? "text-white/75 drop-shadow-[0_1px_2px_rgba(0,0,0,0.9)]"
          : "text-content-faint",
        danger
          ? "hover:bg-danger hover:text-white"
          : immersive
            ? "hover:bg-white/15 hover:text-white"
            : "hover:bg-surface-3 hover:text-content",
      )}
    >
      {children}
    </button>
  );
}

export function TitleBar({ immersive = false }: { immersive?: boolean }) {
  return (
    <div
      data-tauri-drag-region
      className={cn(
        "flex h-9 shrink-0 items-center justify-between",
        immersive
          ? "absolute inset-x-0 top-0 z-50"
          : "relative z-50 border-b border-border-soft bg-base/80 backdrop-blur",
      )}
    >
      {immersive && (
        <div
          data-tauri-drag-region
          className="pointer-events-none absolute inset-x-0 top-0 h-20 bg-gradient-to-b from-black/85 via-black/45 to-transparent"
        />
      )}

      <div className="w-24" />

      <div
        data-tauri-drag-region
        className={cn(
          "pointer-events-none absolute inset-x-0 text-center font-pixel text-[11px] tracking-[0.35em]",
          immersive
            ? "text-white/70 drop-shadow-[0_1px_3px_rgba(0,0,0,0.95)]"
            : "text-content-faint",
        )}
      >
        BASALT
      </div>

      <div className="relative flex items-center">
        <RunningPill immersive={immersive} />
        <ActivityCenter immersive={immersive} />
        <Control label="Minimize" immersive={immersive} onClick={() => win.minimize()}>
          <Minus className="size-3.5" />
        </Control>
        <Control label="Maximize" immersive={immersive} onClick={() => win.toggleMaximize()}>
          <Square className="size-3" />
        </Control>
        <Control label="Close" danger immersive={immersive} onClick={() => win.close()}>
          <X className="size-3.5" />
        </Control>
      </div>
    </div>
  );
}
