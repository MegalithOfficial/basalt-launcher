import { getCurrentWindow } from "@tauri-apps/api/window";

const win = getCurrentWindow();

type Direction =
  | "North"
  | "South"
  | "East"
  | "West"
  | "NorthEast"
  | "NorthWest"
  | "SouthEast"
  | "SouthWest";

const GRIPS: Array<{ direction: Direction; className: string }> = [
  { direction: "North", className: "inset-x-3 top-0 h-1.5 cursor-n-resize" },
  { direction: "South", className: "inset-x-3 bottom-0 h-1.5 cursor-s-resize" },
  { direction: "West", className: "inset-y-3 left-0 w-1.5 cursor-w-resize" },
  { direction: "East", className: "inset-y-3 right-0 w-1.5 cursor-e-resize" },
  { direction: "NorthWest", className: "left-0 top-0 size-3 cursor-nw-resize" },
  { direction: "NorthEast", className: "right-0 top-0 size-3 cursor-ne-resize" },
  { direction: "SouthWest", className: "bottom-0 left-0 size-3 cursor-sw-resize" },
  { direction: "SouthEast", className: "bottom-0 right-0 size-3 cursor-se-resize" },
];

export function WindowFrame({ enabled }: { enabled: boolean }) {
  if (!enabled) return null;

  return (
    <div className="pointer-events-none fixed inset-0 z-100">
      {GRIPS.map((grip) => (
        <div
          key={grip.direction}
          onMouseDown={(event) => {
            if (event.button !== 0) return;
            event.preventDefault();
            void win.startResizeDragging(grip.direction);
          }}
          className={`pointer-events-auto absolute ${grip.className}`}
        />
      ))}
    </div>
  );
}
