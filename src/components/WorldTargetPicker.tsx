import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { Check, ChevronDown, Globe2 } from "lucide-react";

import { cn } from "../lib/cn";
import type { WorldSummary } from "../lib/types";

const EDGE = 12;

export function WorldTargetPicker({
  worlds,
  selected,
  onSelect,
}: {
  worlds: WorldSummary[];
  selected: string | null;
  onSelect: (world: string | null) => void;
}) {
  const [open, setOpen] = useState(false);
  const [rect, setRect] = useState<DOMRect | null>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onPointerDown = (event: PointerEvent) => {
      const target = event.target as Node;
      if (!triggerRef.current?.contains(target) && !menuRef.current?.contains(target)) {
        setOpen(false);
      }
    };
    const close = () => setOpen(false);
    const onScroll = (event: Event) => {
      if (menuRef.current?.contains(event.target as Node)) return;
      setOpen(false);
    };
    window.addEventListener("pointerdown", onPointerDown);
    window.addEventListener("resize", close);
    window.addEventListener("scroll", onScroll, true);
    return () => {
      window.removeEventListener("pointerdown", onPointerDown);
      window.removeEventListener("resize", close);
      window.removeEventListener("scroll", onScroll, true);
    };
  }, [open]);

  const chosen = worlds.find((world) => world.folder_name === selected);
  const label = selected ? (chosen?.name ?? selected) : "Ask each time";

  const choose = (world: string | null) => {
    onSelect(world);
    setOpen(false);
  };

  return (
    <>
      <button
        ref={triggerRef}
        onClick={() => {
          if (!open && triggerRef.current) setRect(triggerRef.current.getBoundingClientRect());
          setOpen((value) => !value);
        }}
        title="Where datapacks go"
        className={cn(
          "inline-flex h-8 items-center gap-2 rounded-lg border px-3 text-xs font-medium transition-colors",
          selected
            ? "border-(--accent)/40 bg-(--accent)/10 text-(--accent) hover:bg-(--accent)/20"
            : "border-dashed border-border text-content-faint hover:text-content",
        )}
      >
        <Globe2 className="size-3.5" />
        <span className="max-w-56 truncate">{label}</span>
        <ChevronDown className={cn("size-3 transition-transform", open && "rotate-180")} />
      </button>

      {open &&
        rect &&
        createPortal(
          <div
            ref={menuRef}
            style={{
              top: Math.min(rect.bottom + 6, window.innerHeight - 320),
              left: Math.max(EDGE, Math.min(rect.right - 288, window.innerWidth - 288 - EDGE)),
            }}
            className="fixed z-70 max-h-72 w-72 overflow-y-auto rounded-xl border border-border bg-surface p-1 shadow-2xl"
          >
            <button
              onClick={() => choose(null)}
              className="flex w-full items-center gap-2.5 rounded-lg px-2.5 py-2 text-left text-xs text-content-muted transition-colors hover:bg-surface-2 hover:text-content"
            >
              <span className="min-w-0 flex-1">Ask each time</span>
              {!selected && <Check className="size-3.5 shrink-0 text-(--accent)" />}
            </button>

            {worlds.length > 0 && <div className="my-1 h-px bg-border-soft" />}

            {worlds.map((world) => (
              <button
                key={world.folder_name}
                onClick={() => choose(world.folder_name)}
                className="flex w-full items-center gap-2.5 rounded-lg px-2.5 py-2 text-left transition-colors hover:bg-surface-2"
              >
                <span className="min-w-0 flex-1">
                  <span className="block truncate text-xs font-medium text-content">
                    {world.name}
                  </span>
                  <span className="block truncate font-mono text-[10px] text-content-faint">
                    {world.folder_name}
                  </span>
                </span>
                {selected === world.folder_name && (
                  <Check className="size-3.5 shrink-0 text-(--accent)" />
                )}
              </button>
            ))}

            {worlds.length === 0 && (
              <p className="px-2.5 py-3 text-xs text-content-faint">
                This instance has no worlds yet.
              </p>
            )}
          </div>,
          document.body,
        )}
    </>
  );
}
