import { motion } from "motion/react";
import { Boxes, ChevronsUpDown, Play, ScrollText, Settings, UserCircle2 } from "lucide-react";

import { cn } from "../lib/cn";
import type { View } from "../lib/types";
import { useStore } from "../store";

const NAV: Array<{ id: View; label: string; icon: typeof Play }> = [
  { id: "home", label: "Play", icon: Play },
  { id: "instances", label: "Instances", icon: Boxes },
  { id: "settings", label: "Settings", icon: Settings },
];

export function Sidebar() {
  const view = useStore((s) => s.view);
  const setView = useStore((s) => s.setView);
  const activeAccount = useStore((s) => s.accounts.find((a) => a.active));

  return (
    <aside className="flex w-60 shrink-0 flex-col border-r border-border-soft bg-surface/50">
      <div className="flex items-center gap-3 px-5 pb-6 pt-5">
        <div className="grid size-9 place-items-center rounded-lg bg-gradient-to-br from-lava-bright to-lava shadow-lg shadow-lava/25">
          <span className="font-pixel text-[15px] leading-none text-black">B</span>
        </div>
        <div className="leading-none">
          <div className="font-pixel text-[13px] tracking-wide text-content">BASALT</div>
          <div className="mt-1 text-[10px] font-semibold uppercase tracking-[0.2em] text-content-faint">
            Launcher
          </div>
        </div>
      </div>

      <nav className="flex flex-1 flex-col gap-1 px-3">
        {NAV.map(({ id, label, icon: Icon }) => {
          const active = view === id;
          return (
            <button
              key={id}
              onClick={() => setView(id)}
              className={cn(
                "group relative flex items-center gap-3 rounded-lg px-3 py-2.5 text-sm font-medium transition-colors",
                active ? "text-black" : "text-content-muted hover:bg-surface-2 hover:text-content",
              )}
            >
              {active && (
                <motion.span
                  layoutId="nav-active"
                  className="absolute inset-0 rounded-lg bg-gradient-to-b from-lava to-lava-deep shadow-lg shadow-lava/20"
                  transition={{ type: "spring", stiffness: 500, damping: 38 }}
                />
              )}
              <Icon
                className={cn(
                  "relative z-10 size-[18px] transition-colors",
                  active ? "text-black" : "text-content-faint group-hover:text-content-muted",
                )}
              />
              <span className="relative z-10">{label}</span>
            </button>
          );
        })}
      </nav>

      <div className="flex flex-col gap-2 px-3 pb-4">
        <button
          className="flex items-center justify-center gap-2 rounded-lg border border-border-soft bg-surface-2/60 px-3 py-2 text-xs font-medium text-content-muted transition-colors hover:bg-surface-2 hover:text-content"
        >
          <ScrollText className="size-3.5" />
          View last log
        </button>

        <button
          onClick={() => setView("accounts")}
          className="flex items-center gap-2.5 rounded-lg border border-border-soft bg-surface-2/70 px-2.5 py-2 text-left transition-colors hover:bg-surface-2"
        >
          <div className="grid size-8 shrink-0 place-items-center rounded-md bg-surface-3 text-content-faint">
            <UserCircle2 className="size-[18px]" />
          </div>
          <div className="min-w-0 flex-1 leading-tight">
            <div className="truncate text-xs font-semibold text-content-muted">
              {activeAccount?.name ?? "No account"}
            </div>
            <div className="truncate text-[11px] text-content-faint">
              {activeAccount ? "Switch or manage" : "Click to sign in"}
            </div>
          </div>
          <ChevronsUpDown className="size-3.5 shrink-0 text-content-faint" />
        </button>
      </div>
    </aside>
  );
}
