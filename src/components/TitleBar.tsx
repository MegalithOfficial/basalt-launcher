import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, Square, X } from "lucide-react";

const win = getCurrentWindow();

function Control({
  onClick,
  label,
  danger,
  children,
}: {
  onClick: () => void;
  label: string;
  danger?: boolean;
  children: React.ReactNode;
}) {
  return (
    <button
      aria-label={label}
      onClick={onClick}
      className={[
        "grid h-8 w-11 place-items-center text-content-faint transition-colors",
        danger ? "hover:bg-danger hover:text-white" : "hover:bg-surface-3 hover:text-content",
      ].join(" ")}
    >
      {children}
    </button>
  );
}

export function TitleBar() {
  return (
    <div
      data-tauri-drag-region
      className="relative flex h-9 shrink-0 items-center justify-between border-b border-border-soft bg-base/80 backdrop-blur"
    >
      <div className="w-24" />

      <div
        data-tauri-drag-region
        className="pointer-events-none absolute inset-x-0 text-center font-pixel text-[11px] tracking-[0.35em] text-content-faint"
      >
        BASALT
      </div>

      <div className="flex items-center">
        <Control label="Minimize" onClick={() => win.minimize()}>
          <Minus className="size-3.5" />
        </Control>
        <Control label="Maximize" onClick={() => win.toggleMaximize()}>
          <Square className="size-3" />
        </Control>
        <Control label="Close" danger onClick={() => win.close()}>
          <X className="size-3.5" />
        </Control>
      </div>
    </div>
  );
}
