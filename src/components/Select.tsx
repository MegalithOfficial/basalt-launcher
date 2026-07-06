import { useEffect, useRef, useState } from "react";
import { Check, ChevronDown } from "lucide-react";

import { cn } from "../lib/cn";

export function Select({
  value,
  options,
  onChange,
  placeholder = "Select",
}: {
  value: string | null;
  options: string[];
  onChange: (value: string) => void;
  placeholder?: string;
}) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onPointerDown = (e: PointerEvent) => {
      if (rootRef.current && !rootRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    window.addEventListener("pointerdown", onPointerDown);
    return () => window.removeEventListener("pointerdown", onPointerDown);
  }, [open]);

  return (
    <div ref={rootRef} className="relative w-full">
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        className="flex w-full items-center justify-between gap-2 rounded-lg border border-border bg-base px-3 py-2 text-sm text-content outline-none transition-colors focus:border-[var(--accent)]"
      >
        <span className={cn("truncate", !value && "text-content-faint")}>
          {value ?? placeholder}
        </span>
        <ChevronDown
          className={cn(
            "size-4 shrink-0 text-content-faint transition-transform",
            open && "rotate-180",
          )}
        />
      </button>

      {open && (
        <div className="absolute inset-x-0 top-full z-30 mt-1 max-h-56 overflow-y-auto rounded-lg border border-border bg-surface-2 py-1 shadow-2xl shadow-black/50">
          {options.map((option) => (
            <button
              key={option}
              type="button"
              onClick={() => {
                onChange(option);
                setOpen(false);
              }}
              className={cn(
                "flex w-full items-center justify-between px-3 py-2 text-left text-sm transition-colors",
                option === value
                  ? "bg-[var(--accent-glow)] text-content"
                  : "text-content-muted hover:bg-surface-3 hover:text-content",
              )}
            >
              <span className="truncate font-mono text-xs">{option}</span>
              {option === value && <Check className="size-3.5 shrink-0 text-[var(--accent)]" />}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
