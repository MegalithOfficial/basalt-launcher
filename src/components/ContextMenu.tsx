import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import type { LucideIcon } from "lucide-react";

import { cn } from "../lib/cn";

export interface MenuItem {
  label: string;
  icon: LucideIcon;
  onSelect: () => void;
  disabled?: boolean;
  danger?: boolean;
  separated?: boolean;
}

export interface MenuState {
  x: number;
  y: number;
  header?: string;
  items: MenuItem[];
}

const EDGE = 12;

export function ContextMenu({ menu, onClose }: { menu: MenuState | null; onClose: () => void }) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<{ left: number; top: number } | null>(null);

  useLayoutEffect(() => {
    setPos(null);
  }, [menu]);

  useLayoutEffect(() => {
    if (!menu || !ref.current) return;
    const rect = ref.current.getBoundingClientRect();
    const maxLeft = window.innerWidth - rect.width - EDGE;
    const mirrored = menu.x - rect.width;
    const left =
      menu.x > maxLeft && mirrored >= EDGE ? mirrored : Math.min(menu.x, maxLeft);
    const top = Math.min(menu.y, window.innerHeight - rect.height - EDGE);
    setPos({ left: Math.max(EDGE, left), top: Math.max(EDGE, top) });
  }, [menu]);

  useEffect(() => {
    if (!menu) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    const onPointer = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) onClose();
    };
    window.addEventListener("keydown", onKey);
    window.addEventListener("mousedown", onPointer, true);
    window.addEventListener("blur", onClose);
    window.addEventListener("resize", onClose);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("mousedown", onPointer, true);
      window.removeEventListener("blur", onClose);
      window.removeEventListener("resize", onClose);
    };
  }, [menu, onClose]);

  if (!menu) return null;

  return createPortal(
    <div
      ref={ref}
      role="menu"
      onContextMenu={(e) => e.preventDefault()}
      style={{ left: pos?.left ?? menu.x, top: pos?.top ?? menu.y }}
      className={cn(
        "fixed z-100 min-w-52 origin-top-left rounded-xl border border-border bg-surface p-1 shadow-2xl shadow-black/60 transition-[opacity,transform] duration-100",
        pos ? "scale-100 opacity-100" : "scale-95 opacity-0",
      )}
    >
      {menu.header && (
        <div className="truncate px-2.5 pb-1.5 pt-1 text-[11px] font-semibold text-content-faint">
          {menu.header}
        </div>
      )}
      {menu.items.map((item) => (
        <div key={item.label} className="contents">
          {item.separated && <div className="my-1 h-px bg-border-soft" />}
          <button
            role="menuitem"
            disabled={item.disabled}
            onClick={() => {
              item.onSelect();
              onClose();
            }}
            className={cn(
              "flex w-full items-center gap-2.5 rounded-lg px-2.5 py-1.5 text-left text-[12.5px] font-medium outline-none transition-colors",
              item.disabled
                ? "cursor-not-allowed text-content-faint/40"
                : item.danger
                  ? "text-danger hover:bg-danger/15"
                  : "text-content-muted hover:bg-surface-2 hover:text-content",
            )}
          >
            <item.icon className="size-4 shrink-0" />
            <span className="truncate">{item.label}</span>
          </button>
        </div>
      ))}
    </div>,
    document.body,
  );
}

export function useContextMenu() {
  const [menu, setMenu] = useState<MenuState | null>(null);

  const open = useCallback(
    (
      e: React.MouseEvent,
      items: MenuItem[],
      header?: string,
      opts?: { fromElement?: boolean; below?: boolean },
    ) => {
      e.preventDefault();
      e.stopPropagation();
      const anchored = opts?.fromElement || opts?.below;
      const rect = anchored
        ? (e.currentTarget as HTMLElement).getBoundingClientRect()
        : null;
      if (rect && opts?.below) {
        setMenu({ x: rect.right, y: rect.bottom + 6, items, header });
        return;
      }
      setMenu({
        x: rect ? rect.right + 10 : e.clientX,
        y: rect ? rect.top - 6 : e.clientY,
        items,
        header,
      });
    },
    [],
  );

  const close = useCallback(() => setMenu(null), []);

  return { menu, open, close };
}
