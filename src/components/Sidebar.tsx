import { motion } from "motion/react";
import { Boxes, Play, Settings, UserCircle2 } from "lucide-react";

import { cn } from "../lib/cn";
import type { View } from "../lib/types";
import { useStore } from "../store";

const NAV: Array<{ id: View; label: string; icon: typeof Play }> = [
  { id: "home", label: "Play", icon: Play },
  { id: "instances", label: "Instances", icon: Boxes },
  { id: "accounts", label: "Accounts", icon: UserCircle2 },
  { id: "settings", label: "Settings", icon: Settings },
];

export function Sidebar() {
  const view = useStore((s) => s.view);
  const setView = useStore((s) => s.setView);

  return (
    <aside className="flex w-60 shrink-0 flex-col border-r border-border-soft bg-surface/60 backdrop-blur">
      <div className="flex items-center gap-3 px-5 pb-5 pt-6">
        <div className="grid size-9 place-items-center rounded-lg bg-gradient-to-br from-lava to-lava-deep shadow-lg shadow-lava/20">
          <span className="font-pixel text-[15px] leading-none text-black">B</span>
        </div>
        <div className="leading-tight">
          <div className="font-pixel text-sm tracking-wide text-content">basalt</div>
          <div className="text-[11px] font-medium text-content-faint">launcher</div>
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
                active
                  ? "text-content"
                  : "text-content-muted hover:bg-surface-2 hover:text-content",
              )}
            >
              {active && (
                <motion.span
                  layoutId="nav-active"
                  className="absolute inset-0 -z-10 rounded-lg bg-surface-2 ring-1 ring-border"
                  transition={{ type: "spring", stiffness: 500, damping: 38 }}
                />
              )}
              <Icon
                className={cn(
                  "size-[18px] transition-colors",
                  active ? "text-lava" : "text-content-faint group-hover:text-content-muted",
                )}
              />
              {label}
            </button>
          );
        })}
      </nav>

      <div className="px-3 pb-4">
        <div className="rounded-lg border border-border-soft bg-surface-2/70 px-3 py-2.5">
          <div className="flex items-center gap-2.5">
            <div className="grid size-7 place-items-center rounded-md bg-surface-3 text-content-faint">
              <UserCircle2 className="size-4" />
            </div>
            <div className="min-w-0 leading-tight">
              <div className="truncate text-xs font-semibold text-content-muted">
                No account
              </div>
              <div className="truncate text-[11px] text-content-faint">Sign in to play</div>
            </div>
          </div>
        </div>
      </div>
    </aside>
  );
}
