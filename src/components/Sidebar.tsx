import { motion } from "motion/react";
import { Boxes, Play, ScrollText, Settings, UserCircle2 } from "lucide-react";

import { cn } from "../lib/cn";
import type { View } from "../lib/types";
import { useStore } from "../store";

const NAV: Array<{ id: View; label: string; icon: typeof Play }> = [
  { id: "home", label: "Play", icon: Play },
  { id: "instances", label: "Instances", icon: Boxes },
  { id: "settings", label: "Settings", icon: Settings },
];

function RailButton({
  label,
  active,
  onClick,
  disabled,
  children,
}: {
  label: string;
  active?: boolean;
  onClick?: () => void;
  disabled?: boolean;
  children: React.ReactNode;
}) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      aria-label={label}
      className={cn(
        "group relative grid size-11 place-items-center rounded-xl transition-colors",
        active ? "text-black" : "text-content-faint hover:bg-surface-2 hover:text-content",
        disabled && "cursor-not-allowed opacity-40 hover:bg-transparent hover:text-content-faint",
      )}
    >
      {active && (
        <motion.span
          layoutId="rail-active"
          className="absolute inset-0 rounded-xl shadow-lg shadow-[var(--accent-glow)] transition-colors duration-500 [background:linear-gradient(to_bottom,var(--accent),var(--accent-deep))]"
          transition={{ type: "spring", stiffness: 500, damping: 38 }}
        />
      )}
      <span className="relative z-10">{children}</span>
    </button>
  );
}

export function Sidebar() {
  const view = useStore((s) => s.view);
  const setView = useStore((s) => s.setView);
  const activeAccount = useStore((s) => s.accounts.find((a) => a.active));
  const running = useStore((s) => s.running);
  const openConsole = useStore((s) => s.openConsole);

  const latestRun = Object.values(running).sort((a, b) => b.started_at - a.started_at)[0];
  const anyRunning = Object.values(running).some((r) => r.state === "running");

  return (
    <aside className="flex w-[68px] shrink-0 flex-col items-center border-r border-border-soft bg-surface/40 py-4">
      <button
        onClick={() => setView("home")}
        aria-label="Basalt"
        className="group relative size-10 overflow-hidden rounded-xl transition-transform hover:scale-105 active:scale-95"
      >
        <img
          src="/logo.png"
          alt="Basalt"
          className="size-full scale-[1.45] object-cover"
          draggable={false}
        />
      </button>

      <nav className="mt-6 flex flex-col gap-1.5">
        {NAV.map(({ id, label, icon: Icon }) => (
          <RailButton key={id} label={label} active={view === id} onClick={() => setView(id)}>
            <Icon className="size-[19px]" />
          </RailButton>
        ))}
      </nav>

      <div className="flex-1" />

      <div className="flex flex-col items-center gap-1.5">
        <RailButton
          label={anyRunning ? "Console · running" : "Last log"}
          active={view === "console"}
          disabled={!latestRun}
          onClick={() => latestRun && openConsole(latestRun.running_id)}
        >
          <span className="relative">
            <ScrollText className="size-[19px]" />
            {anyRunning && view !== "console" && (
              <span className="absolute -right-1 -top-1 size-2 rounded-full bg-ok ring-2 ring-surface" />
            )}
          </span>
        </RailButton>

        <RailButton
          label={activeAccount ? activeAccount.name : "Sign in"}
          active={view === "accounts"}
          onClick={() => setView("accounts")}
        >
          {activeAccount ? (
            <span
              className={cn(
                "grid size-7 place-items-center rounded-full text-[11px] font-bold",
                view === "accounts"
                  ? "bg-black/25 text-black"
                  : "bg-surface-3 text-content group-hover:bg-border",
              )}
            >
              {activeAccount.name.slice(0, 1).toUpperCase()}
            </span>
          ) : (
            <UserCircle2 className="size-[19px]" />
          )}
        </RailButton>
      </div>
    </aside>
  );
}
