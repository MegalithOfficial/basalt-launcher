import { useEffect, useState } from "react";
import { Loader2, TriangleAlert } from "lucide-react";

import { cn } from "../lib/cn";
import { Modal } from "./Modal";

export function ConfirmDialog({
  open,
  title,
  description,
  children,
  tone = "danger",
  presentation = "modal",
  nested,
  confirmLabel = "Delete",
  cancelLabel = "Cancel",
  confirmIcon,
  requireText,
  onConfirm,
  onCancel,
}: {
  open: boolean;
  title: React.ReactNode;
  description?: React.ReactNode;
  /** Extra content between the description and the buttons, modal only. */
  children?: React.ReactNode;
  tone?: "danger" | "warn";
  /** Inline sits over its own card; modal covers the window. */
  presentation?: "modal" | "inline";
  nested?: boolean;
  confirmLabel?: string;
  cancelLabel?: string;
  confirmIcon?: React.ReactNode;
  /** When set, this exact text has to be typed before the action unlocks. */
  requireText?: string;
  onConfirm: () => Promise<void> | void;
  onCancel: () => void;
}) {
  const [typed, setTyped] = useState("");
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);

  useEffect(() => {
    if (!open) {
      setTyped("");
      setFailure(null);
    }
  }, [open]);

  const locked = !!requireText && typed.trim() !== requireText;

  const close = () => {
    if (!busy) onCancel();
  };

  const run = async () => {
    setBusy(true);
    setFailure(null);
    try {
      await onConfirm();
    } catch (error) {
      setFailure(String(error));
    } finally {
      setBusy(false);
    }
  };

  if (presentation === "inline") {
    if (!open) return null;
    return (
      <div className="absolute inset-0 z-10 flex items-center gap-3 rounded-2xl border border-danger/40 bg-base/95 px-3.5 backdrop-blur">
        <TriangleAlert className="size-4 shrink-0 text-danger" />
        <div className="min-w-0 flex-1">
          <div className="truncate text-xs font-semibold text-content">{title}</div>
          {description && (
            <div className="text-[11px] text-content-faint">{description}</div>
          )}
        </div>
        <button
          onClick={close}
          disabled={busy}
          className="shrink-0 rounded-lg px-2.5 py-1.5 text-xs font-medium text-content-muted transition-colors hover:text-content disabled:opacity-50"
        >
          {cancelLabel}
        </button>
        <button
          onClick={run}
          disabled={busy}
          className="inline-flex shrink-0 items-center gap-1.5 rounded-lg border border-danger/40 bg-danger/15 px-2.5 py-1.5 text-xs font-semibold text-danger transition-colors hover:bg-danger/25 disabled:opacity-50"
        >
          {busy ? <Loader2 className="size-3.5 animate-spin" /> : confirmIcon}
          {confirmLabel}
        </button>
      </div>
    );
  }

  return (
    <Modal open={open} onClose={close} size="md" nested={nested} dismissable={!busy}>
      <div className="flex items-start gap-3 border-b border-border-soft px-5 py-4">
        <TriangleAlert
          className={cn(
            "mt-0.5 size-4 shrink-0",
            tone === "danger" ? "text-danger" : "text-warn",
          )}
        />
        <div className="min-w-0">
          <h2 className="font-display text-[1rem] font-semibold text-content">{title}</h2>
          {description && (
            <div className="mt-1 text-xs leading-relaxed text-content-muted">{description}</div>
          )}
        </div>
      </div>

      {children && <div className="border-b border-border-soft px-5 py-4">{children}</div>}

      {failure && (
        <div className="border-b border-border-soft px-5 py-4">
          <div className="rounded-lg border border-danger/30 bg-danger/10 px-3 py-2 text-xs text-danger">
            {failure}
          </div>
        </div>
      )}

      {requireText && (
        <div className="border-b border-border-soft px-5 py-4">
          <label className="block text-xs text-content-faint">
            Type <span className="font-medium text-content">{requireText}</span> to confirm
          </label>
          <input
            value={typed}
            onChange={(e) => setTyped(e.target.value)}
            autoFocus
            spellCheck={false}
            className="mt-1.5 w-full rounded-lg border border-border bg-base px-3 py-2 text-sm text-content outline-none transition-colors focus:border-danger"
          />
        </div>
      )}

      <div className="flex items-center justify-end gap-2 px-5 py-4">
        <button
          onClick={close}
          disabled={busy}
          className="rounded-lg px-3 py-2 text-sm font-medium text-content-muted transition-colors hover:text-content disabled:opacity-50"
        >
          {cancelLabel}
        </button>
        <button
          onClick={run}
          disabled={busy || locked}
          className={cn(
            "inline-flex items-center gap-1.5 rounded-lg bg-danger/15 px-4 py-2 text-sm font-semibold text-danger transition-colors hover:bg-danger/25",
            (busy || locked) && "cursor-not-allowed opacity-50",
          )}
        >
          {busy && <Loader2 className="size-3.5 animate-spin" />}
          {confirmLabel}
        </button>
      </div>
    </Modal>
  );
}
