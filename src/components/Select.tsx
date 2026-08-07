import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { Check, ChevronDown } from "lucide-react";

import { cn } from "../lib/cn";

const MENU_MAX_HEIGHT = 240;

export function Select({
  value,
  options,
  onChange,
  placeholder = "Select",
  label,
  compact,
}: {
  value: string | null;
  options: string[];
  onChange: (value: string) => void;
  placeholder?: string;
  label?: string;
  compact?: boolean;
}) {
  const [open, setOpen] = useState(false);
  const [rect, setRect] = useState<DOMRect | null>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);

  const toggle = () => {
    if (!open && triggerRef.current) {
      setRect(triggerRef.current.getBoundingClientRect());
    }
    setOpen((v) => !v);
  };

  useEffect(() => {
    if (!open) return;
    const onPointerDown = (e: PointerEvent) => {
      const target = e.target as Node;
      if (!triggerRef.current?.contains(target) && !menuRef.current?.contains(target)) {
        setOpen(false);
      }
    };
    const close = () => setOpen(false);
    const onScroll = (e: Event) => {
      if (menuRef.current?.contains(e.target as Node)) return;
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

  const openUp =
    rect !== null &&
    window.innerHeight - rect.bottom < MENU_MAX_HEIGHT + 8 &&
    rect.top > MENU_MAX_HEIGHT + 8;

  return (
    <>
      <button
        ref={triggerRef}
        type="button"
        onClick={toggle}
        className={cn(
          "flex w-full items-center justify-between gap-2 rounded-lg border border-border bg-void text-content outline-none transition-colors focus:border-(--accent)",
          compact ? "px-2.5 py-1.5 text-xs" : "px-3 py-2 text-sm",
        )}
      >
        <span className="flex min-w-0 items-center gap-1.5">
          {label && <span className="shrink-0 text-content-faint">{label}</span>}
          <span className={cn("truncate", !value && "text-content-faint")}>
            {value ?? placeholder}
          </span>
        </span>
        <ChevronDown
          className={cn(
            "shrink-0 text-content-faint transition-transform",
            compact ? "size-3.5" : "size-4",
            open && "rotate-180",
          )}
        />
      </button>

      {open &&
        rect &&
        createPortal(
          <div
            ref={menuRef}
            style={{
              position: "fixed",
              left: rect.left,
              width: rect.width,
              maxHeight: MENU_MAX_HEIGHT,
              ...(openUp
                ? { bottom: window.innerHeight - rect.top + 4 }
                : { top: rect.bottom + 4 }),
            }}
            className="z-100 overflow-y-auto rounded-lg border border-border bg-surface-2 py-1 shadow-2xl shadow-black/50"
          >
            {options.map((option) => (
              <button
                key={option}
                type="button"
                onPointerDown={(e) => e.stopPropagation()}
                onClick={() => {
                  onChange(option);
                  setOpen(false);
                }}
                className={cn(
                  "flex w-full items-center justify-between text-left transition-colors",
                  compact ? "px-2.5 py-1.5 text-xs" : "px-3 py-2 text-sm",
                  option === value
                    ? "bg-(--accent-glow) text-content"
                    : "text-content-muted hover:bg-surface-3 hover:text-content",
                )}
              >
                <span className={cn("truncate", compact ? "text-xs" : "font-mono text-xs")}>
                  {option}
                </span>
                {option === value && (
                  <Check className="size-3.5 shrink-0 text-(--accent)" />
                )}
              </button>
            ))}
          </div>,
          document.body,
        )}
    </>
  );
}
