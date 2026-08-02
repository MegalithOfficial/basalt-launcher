import { useCallback, useEffect, useRef, useState } from "react";

import { cn } from "../lib/cn";

const FLOOR = 512;
const STEP = 256;

const CANDIDATES = [1024, 2048, 3072, 4096, 6144, 8192, 12288, 16384, 24576, 32768, 49152];

const LABEL_GAP = 10;

function label(mb: number) {
  if (mb % 1024 === 0) return `${mb / 1024} GB`;
  return mb >= 1024 ? `${(mb / 1024).toFixed(1)} GB` : `${mb} MB`;
}

export function MemoryRange({
  min,
  max,
  ceiling,
  available,
  onChange,
}: {
  min: number;
  max: number;
  ceiling: number;
  available?: number;
  onChange: (min: number, max: number) => void;
}) {
  const trackRef = useRef<HTMLDivElement>(null);
  const [dragging, setDragging] = useState<"min" | "max" | null>(null);

  const span = Math.max(ceiling - FLOOR, STEP);
  const percent = (value: number) =>
    Math.min(100, Math.max(0, ((value - FLOOR) / span) * 100));

  const valueAt = useCallback(
    (clientX: number) => {
      const track = trackRef.current;
      if (!track) return FLOOR;
      const rect = track.getBoundingClientRect();
      const ratio = Math.min(1, Math.max(0, (clientX - rect.left) / rect.width));
      return Math.min(
        ceiling,
        Math.max(FLOOR, Math.round((FLOOR + ratio * span) / STEP) * STEP),
      );
    },
    [ceiling, span],
  );

  const move = useCallback(
    (handle: "min" | "max", clientX: number) => {
      const value = valueAt(clientX);
      if (handle === "min") onChange(Math.min(value, max - STEP), max);
      else onChange(min, Math.max(value, min + STEP));
    },
    [valueAt, onChange, min, max],
  );

  useEffect(() => {
    if (!dragging) return;
    const onMove = (event: PointerEvent) => move(dragging, event.clientX);
    const stop = () => setDragging(null);
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", stop);
    window.addEventListener("pointercancel", stop);
    return () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", stop);
      window.removeEventListener("pointercancel", stop);
    };
  }, [dragging, move]);

  const start = (handle: "min" | "max", event: React.PointerEvent) => {
    event.preventDefault();
    event.stopPropagation();
    setDragging(handle);
    move(handle, event.clientX);
  };

  const tight = available != null && max > available;

  const ticks: number[] = [];
  let lastLabel = 0;
  for (const candidate of CANDIDATES) {
    if (candidate <= FLOOR + STEP || candidate >= ceiling - STEP) continue;
    const at = percent(candidate);
    if (at - lastLabel < LABEL_GAP || 100 - at < LABEL_GAP) continue;
    ticks.push(candidate);
    lastLabel = at;
  }

  const handle = (which: "min" | "max", value: number) => (
    <div
      role="slider"
      tabIndex={0}
      aria-label={which === "min" ? "Minimum memory" : "Maximum memory"}
      aria-valuemin={FLOOR}
      aria-valuemax={ceiling}
      aria-valuenow={value}
      onPointerDown={(event) => start(which, event)}
      onKeyDown={(event) => {
        const delta = event.key === "ArrowLeft" ? -STEP : event.key === "ArrowRight" ? STEP : 0;
        if (!delta) return;
        event.preventDefault();
        if (which === "min") onChange(Math.min(Math.max(FLOOR, min + delta), max - STEP), max);
        else onChange(min, Math.max(Math.min(ceiling, max + delta), min + STEP));
      }}
      style={{ left: `${percent(value)}%` }}
      className={cn(
        "group absolute top-1/2 z-10 grid size-5 -translate-x-1/2 -translate-y-1/2 cursor-grab place-items-center rounded-full border border-border bg-surface shadow-lg shadow-black/40 outline-none transition-transform",
        "hover:scale-110 focus-visible:ring-2 focus-visible:ring-(--accent)/60",
        dragging === which && "scale-110 cursor-grabbing",
      )}
    >
      <span className="size-2 rounded-full bg-(--accent)" />
      <span
        className={cn(
          "pointer-events-none absolute -top-8 whitespace-nowrap rounded-md border border-border bg-surface-3 px-1.5 py-0.5 text-[10px] font-medium tabular-nums text-content opacity-0 shadow-lg transition-opacity",
          dragging === which && "opacity-100",
          "group-hover:opacity-100",
        )}
      >
        {label(value)}
      </span>
    </div>
  );

  return (
    <div className="w-full select-none">
      <div
        ref={trackRef}
        onPointerDown={(event) => {
          const value = valueAt(event.clientX);
          start(Math.abs(value - min) <= Math.abs(value - max) ? "min" : "max", event);
        }}
        className="relative h-2 w-full cursor-pointer rounded-full bg-surface-3"
      >
        <div
          className="absolute inset-y-0 rounded-full [background:linear-gradient(to_right,var(--accent-deep),var(--accent))]"
          style={{ left: `${percent(min)}%`, width: `${percent(max) - percent(min)}%` }}
        />

        {available != null && available > FLOOR && available < ceiling && (
          <span
            title={`${label(available)} free right now`}
            style={{ left: `${percent(available)}%` }}
            className={cn(
              "absolute top-1/2 h-4 w-0.5 -translate-x-1/2 -translate-y-1/2 rounded-full transition-colors",
              tight ? "bg-warn" : "bg-content-faint",
            )}
          />
        )}

        {ticks.map((tick) => (
          <span
            key={tick}
            style={{ left: `${percent(tick)}%` }}
            className="absolute top-1/2 h-2.5 w-px -translate-x-1/2 -translate-y-1/2 bg-content-faint/50"
          />
        ))}

        {handle("min", min)}
        {handle("max", max)}
      </div>

      <div className="relative mt-2 h-4">
        <span className="absolute left-0 text-[10px] tabular-nums text-content-faint">
          {label(FLOOR)}
        </span>
        {ticks.map((tick) => (
          <button
            key={tick}
            onClick={() => onChange(Math.min(min, tick - STEP), tick)}
            title={`Set the ceiling to ${label(tick)}`}
            style={{ left: `${percent(tick)}%` }}
            className="absolute -translate-x-1/2 text-[10px] tabular-nums text-content-faint transition-colors hover:text-content"
          >
            {label(tick)}
          </button>
        ))}
        <span className="absolute right-0 text-[10px] tabular-nums text-content-faint">
          {label(ceiling)}
        </span>
      </div>
    </div>
  );
}
