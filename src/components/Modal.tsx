import { createPortal } from "react-dom";
import { AnimatePresence, motion } from "motion/react";
import { ArrowLeft, X } from "lucide-react";

import { cn } from "../lib/cn";
import { useEscape } from "../lib/useEscape";

const SIZES = {
  sm: "max-w-sm",
  md: "max-w-md",
  lg: "max-w-xl",
  xl: "max-w-2xl",
  wide: "max-w-3xl",
  full: "max-w-5xl",
} as const;

export type ModalSize = keyof typeof SIZES;

export function ModalHeader({
  title,
  subtitle,
  icon,
  onBack,
  onClose,
  id,
}: {
  title: React.ReactNode;
  subtitle?: React.ReactNode;
  icon?: React.ReactNode;
  onBack?: () => void;
  onClose?: () => void;
  id?: string;
}) {
  return (
    <div className="flex items-start gap-3 border-b border-border-soft px-5 py-4">
      {onBack && (
        <button
          onClick={onBack}
          aria-label="Back"
          className="-ml-1.5 grid size-8 shrink-0 place-items-center rounded-lg text-content-faint transition-colors hover:bg-surface-3 hover:text-content"
        >
          <ArrowLeft className="size-4" />
        </button>
      )}
      {icon && <div className="mt-0.5 shrink-0">{icon}</div>}
      <div className="min-w-0 flex-1">
        <h2 id={id} className="font-display text-[1rem] font-semibold text-content">
          {title}
        </h2>
        {subtitle && <div className="mt-0.5 text-xs text-content-muted">{subtitle}</div>}
      </div>
      {onClose && (
        <button
          onClick={onClose}
          aria-label="Close"
          className="grid size-8 shrink-0 place-items-center rounded-lg text-content-faint transition-colors hover:bg-surface-3 hover:text-content"
        >
          <X className="size-4" />
        </button>
      )}
    </div>
  );
}

export function ModalBody({
  children,
  className,
}: {
  children: React.ReactNode;
  className?: string;
}) {
  return <div className={cn("min-h-0 flex-1 overflow-y-auto px-5 py-5", className)}>{children}</div>;
}

export function ModalFooter({
  children,
  className,
}: {
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "flex items-center justify-end gap-2 border-t border-border-soft px-5 py-4",
        className,
      )}
    >
      {children}
    </div>
  );
}

export function Modal({
  open,
  onClose,
  size = "md",
  nested,
  dismissable = true,
  labelledBy,
  className,
  variant = "panel",
  backdropClassName,
  children,
}: {
  open: boolean;
  onClose: () => void;
  size?: ModalSize;
  nested?: boolean;
  dismissable?: boolean;
  labelledBy?: string;
  className?: string;
  variant?: "panel" | "bare";
  backdropClassName?: string;
  children: React.ReactNode;
}) {
  useEscape(open && dismissable, onClose);

  return createPortal(
    <AnimatePresence>
      {open && (
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: 0.14 }}
          onClick={() => dismissable && onClose()}
          className={cn(
            "fixed inset-0 grid place-items-center p-6 backdrop-blur-sm",
            backdropClassName ?? "bg-black/60",
            nested ? "z-70" : "z-50",
          )}
        >
          <motion.div
            role="dialog"
            aria-modal="true"
            aria-labelledby={labelledBy}
            initial={{ opacity: 0, scale: 0.97, y: 8 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.97, y: 8 }}
            transition={{ duration: 0.18 }}
            onClick={(e) => e.stopPropagation()}
            className={cn(
              variant === "panel"
                ? "flex max-h-[min(760px,calc(100vh-48px))] w-full flex-col overflow-hidden rounded-2xl border border-border bg-surface shadow-2xl"
                : "flex h-full w-full min-h-0 flex-col items-center justify-center gap-3",
              variant === "panel" ? SIZES[size] : "max-w-none",
              className,
            )}
          >
            {children}
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>,
    document.body,
  );
}
