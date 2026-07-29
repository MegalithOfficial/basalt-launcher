import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { Boxes, Check, ChevronDown, Compass, X } from "lucide-react";

import { cn } from "../lib/cn";
import { loaderLabel } from "../lib/loader";
import { Modal } from "./Modal";
import type { Instance } from "../lib/types";

function InstanceRow({
  instance,
  selected,
  incompatible,
  installed,
  onClick,
}: {
  instance: Instance;
  selected: boolean;
  incompatible?: boolean;
  installed?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      onClick={installed ? undefined : onClick}
      disabled={installed}
      className={cn(
        "flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left transition-colors",
        installed
          ? "cursor-not-allowed opacity-60"
          : selected
            ? "bg-[var(--accent-glow)]"
            : "hover:bg-surface-2",
      )}
    >
      <div
        className={cn(
          "grid size-8 shrink-0 place-items-center rounded-lg bg-surface-3 text-content-faint",
          incompatible && "opacity-50",
        )}
      >
        <Boxes className="size-4" />
      </div>
      <div className="min-w-0 flex-1">
        <div
          className={cn(
            "truncate text-sm font-medium",
            incompatible ? "text-content-muted" : "text-content",
          )}
        >
          {instance.name}
        </div>
        <div className="truncate text-[11px] text-content-faint">
          {instance.version_id} · {loaderLabel(instance)}
        </div>
      </div>
      {installed ? (
        <span className="shrink-0 rounded bg-ok/15 px-1.5 py-0.5 text-[10px] font-medium text-ok">
          Installed
        </span>
      ) : (
        incompatible && (
          <span className="shrink-0 rounded bg-warn/15 px-1.5 py-0.5 text-[10px] font-medium text-warn">
            Mismatch
          </span>
        )
      )}
      {selected && <Check className="size-4 shrink-0 text-[var(--accent)]" />}
    </button>
  );
}

export function InstanceTargetPicker({
  instances,
  selected,
  onSelect,
  onCancel,
  modalFor,
  isCompatible,
  isInstalled,
}: {
  instances: Instance[];
  selected: Instance | null;
  onSelect: (instance: Instance | null) => void;
  onCancel?: () => void;
  modalFor?: string | null;
  isCompatible?: (instance: Instance) => boolean;
  isInstalled?: (instance: Instance) => boolean;
}) {
  const [open, setOpen] = useState(false);
  const [rect, setRect] = useState<DOMRect | null>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const isModal = modalFor != null;

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
  const close = () => (onCancel ? onCancel() : onSelect(null));
  const ranked = isCompatible
    ? [...instances].sort(
        (a, b) => Number(isCompatible(b)) - Number(isCompatible(a)) || a.name.localeCompare(b.name),
      )
    : instances;
  if (isModal) {
    return (
      <Modal open onClose={close} nested>
        <div className="flex items-start justify-between gap-3 border-b border-border-soft px-5 py-4">
          <div className="min-w-0">
            <h2 className="font-display text-base font-semibold text-content">
              Install to which instance?
            </h2>
            <div className="mt-0.5 truncate text-xs text-content-muted">{modalFor}</div>
          </div>
          <button
            onClick={close}
            aria-label="Cancel"
            className="grid size-7 shrink-0 place-items-center rounded-md text-content-faint transition-colors hover:bg-surface-2 hover:text-content"
          >
            <X className="size-4" />
          </button>
        </div>
        <div className="min-h-0 flex-1 overflow-y-auto p-2">
          {ranked.length === 0 ? (
            <div className="px-3 py-8 text-center text-sm text-content-faint">
              Create an instance first.
            </div>
          ) : (
            ranked.map((instance) => (
              <InstanceRow
                key={instance.id}
                instance={instance}
                selected={false}
                incompatible={isCompatible ? !isCompatible(instance) : false}
                installed={isInstalled?.(instance)}
                onClick={() => onSelect(instance)}
              />
            ))
          )}
        </div>
      </Modal>
    );
  }

  return (
    <div className="relative">
      <button
        ref={triggerRef}
        onClick={() => {
          if (!open && triggerRef.current) setRect(triggerRef.current.getBoundingClientRect());
          setOpen((v) => !v);
        }}
        className={cn(
          "inline-flex h-8 items-center gap-2 rounded-lg border px-3 text-xs font-medium transition-colors",
          selected
            ? "border-border bg-surface-2 text-content hover:bg-surface-3"
            : "border-dashed border-border text-content-faint hover:text-content",
        )}
      >
        {selected ? <Boxes className="size-3.5" /> : <Compass className="size-3.5" />}
        <span className="max-w-[14rem] truncate">
          {selected ? selected.name : "Browsing only"}
        </span>
        <ChevronDown className={cn("size-3 transition-transform", open && "rotate-180")} />
      </button>

      {open &&
        rect &&
        createPortal(
          <div
            ref={menuRef}
            style={{
              position: "fixed",
              top: Math.min(rect.bottom + 6, window.innerHeight - 8),
              left: Math.max(8, rect.right - 288),
              maxHeight: Math.max(160, window.innerHeight - rect.bottom - 16),
            }}
            className="z-[80] w-72 overflow-y-auto rounded-xl border border-border bg-surface p-1.5 shadow-2xl shadow-black/50"
          >
            <button
              onClick={() => {
                onSelect(null);
                setOpen(false);
              }}
              className={cn(
                "flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left transition-colors",
                selected ? "hover:bg-surface-2" : "bg-[var(--accent-glow)]",
              )}
            >
              <div className="grid size-8 shrink-0 place-items-center rounded-lg bg-surface-3 text-content-faint">
                <Compass className="size-4" />
              </div>
              <div className="min-w-0 flex-1">
                <div className="truncate text-sm font-medium text-content">Browsing only</div>
                <div className="truncate text-[11px] text-content-faint">
                  Pick an instance when you install
                </div>
              </div>
              {!selected && <Check className="size-4 shrink-0 text-[var(--accent)]" />}
            </button>

            <div className="my-1 h-px bg-border-soft" />

            {instances.length === 0 ? (
              <div className="px-3 py-6 text-center text-xs text-content-faint">
                No instances yet.
              </div>
            ) : (
              instances.map((instance) => (
                <InstanceRow
                  key={instance.id}
                  instance={instance}
                  selected={selected?.id === instance.id}
                  onClick={() => {
                    onSelect(instance);
                    setOpen(false);
                  }}
                />
              ))
            )}
          </div>,
          document.body,
        )}
    </div>
  );
}
