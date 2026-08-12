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
    <div className="flex items-center justify-between gap-4 border-b border-border-soft px-8 py-3.5">
      <div className="flex min-w-0 items-baseline gap-3">
        <h1 className="font-display text-[1rem] font-semibold tracking-tight text-content">
          {title}
        </h1>
        {subtitle && <span className="text-xs text-content-faint">{subtitle}</span>}
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
          "text-black shadow-lg shadow-(color:--accent-glow) [background:linear-gradient(to_bottom,var(--accent),var(--accent-deep))] hover:[background:linear-gradient(to_bottom,var(--accent-bright),var(--accent))]",
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
          checked ? "left-6 bg-black" : "left-1 bg-content-faint",
        )}
      />
    </button>
  );
}

export function SettingGroup({
  title,
  description,
  children,
  className,
}: {
  title?: string;
  description?: string;
  children: ReactNode;
  className?: string;
}) {
  return (
    <section className={cn("break-inside-avoid", className)}>
      {title && (
        <div className="mb-2 min-h-9 px-1">
          <h2 className="font-display text-sm font-semibold text-content">{title}</h2>
          {description && <p className="mt-0.5 text-xs text-content-muted">{description}</p>}
        </div>
      )}
      <div className="divide-y divide-border-soft rounded-2xl border border-border-soft bg-surface-2/60">
        {children}
      </div>
    </section>
  );
}

export function SettingRow({
  label,
  hint,
  children,
  stacked,
  action,
}: {
  label: string;
  hint?: string;
  children?: ReactNode;
  stacked?: boolean;
  action?: ReactNode;
}) {
  if (stacked) {
    return (
      <div className="px-5 py-4">
        <div className="flex items-center justify-between gap-3">
          <div className="text-sm font-medium text-content">{label}</div>
          {action}
        </div>
        {hint && <div className="mt-0.5 wrap-break-word text-xs text-content-faint">{hint}</div>}
        {children && <div className="mt-3 flex items-center gap-2">{children}</div>}
      </div>
    );
  }
  return (
    <div className="flex items-center justify-between gap-5 px-5 py-4">
      <div className="min-w-0">
        <div className="text-sm font-medium text-content">{label}</div>
        {hint && <div className="mt-0.5 wrap-break-word text-xs text-content-faint">{hint}</div>}
      </div>
      <div className="flex shrink-0 items-center gap-2">{children}</div>
    </div>
  );
}
