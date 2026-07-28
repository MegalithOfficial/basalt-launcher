import type { ReactNode } from "react";

import { cn } from "../lib/cn";

export function PageHeader({
  title,
  subtitle,
  actions,
}: {
  title: string;
  subtitle?: ReactNode;
  actions?: ReactNode;
}) {
  return (
    <div className="flex items-end justify-between gap-4 border-b border-border-soft px-8 py-6">
      <div className="min-w-0">
        <h1 className="font-display text-2xl font-semibold tracking-tight text-content">
          {title}
        </h1>
        {subtitle && <div className="mt-1 text-sm text-content-muted">{subtitle}</div>}
      </div>
      {actions}
    </div>
  );
}

export function EmptyState({
  icon,
  title,
  description,
  action,
}: {
  icon: ReactNode;
  title: string;
  description: string;
  action?: ReactNode;
}) {
  return (
    <div className="grid flex-1 place-items-center px-8">
      <div className="flex max-w-sm flex-col items-center text-center">
        <div className="mb-4 grid size-14 place-items-center rounded-2xl border border-border-soft bg-surface-2 text-content-faint">
          {icon}
        </div>
        <h2 className="font-display text-lg font-semibold text-content">{title}</h2>
        <p className="mt-1.5 text-sm text-content-muted">{description}</p>
        {action && <div className="mt-5">{action}</div>}
      </div>
    </div>
  );
}

export function Button({
  children,
  onClick,
  variant = "primary",
  className,
  type = "button",
  disabled,
}: {
  children: ReactNode;
  onClick?: () => void;
  variant?: "primary" | "ghost";
  className?: string;
  type?: "button" | "submit";
  disabled?: boolean;
}) {
  return (
    <button
      type={type}
      onClick={onClick}
      disabled={disabled}
      className={cn(
        "inline-flex items-center justify-center gap-2 rounded-lg px-4 py-2 text-sm font-semibold transition-all active:scale-[0.98] disabled:cursor-not-allowed disabled:opacity-50",
        variant === "primary" &&
          "text-black shadow-lg shadow-[var(--accent-glow)] [background:linear-gradient(to_bottom,var(--accent),var(--accent-deep))] hover:[background:linear-gradient(to_bottom,var(--accent-bright),var(--accent))]",
        variant === "ghost" &&
          "border border-border bg-surface-2 text-content hover:bg-surface-3",
        className,
      )}
    >
      {children}
    </button>
  );
}

export function Toggle({
  checked,
  onChange,
  label,
  disabled,
}: {
  checked: boolean;
  onChange: (value: boolean) => void;
  label: string;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={cn(
        "relative h-6 w-11 shrink-0 rounded-full border transition-colors disabled:cursor-not-allowed disabled:opacity-50",
        checked
          ? "border-transparent [background:linear-gradient(to_bottom,var(--accent),var(--accent-deep))]"
          : "border-border bg-surface-3",
      )}
    >
      <span
        className={cn(
          "absolute top-1/2 size-4 -translate-y-1/2 rounded-full transition-all",
          checked ? "left-[1.5rem] bg-black" : "left-1 bg-content-faint",
        )}
      />
    </button>
  );
}
