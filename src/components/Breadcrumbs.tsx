import { ChevronRight } from "lucide-react";

import { cn } from "../lib/cn";
import { logoSrc } from "../lib/media";
import type { View } from "../lib/types";
import { useStore } from "../store";

const LABELS: Record<View, string> = {
  home: "Play",
  instances: "Instances",
  accounts: "Accounts",
  settings: "Settings",
  instance: "Instance",
  servers: "Servers",
  server: "Server",
  discover: "Discover",
  project: "Project",
  stats: "Stats",
  logs: "Logs",
};

export function Breadcrumbs({ immersive }: { immersive?: boolean }) {
  const view = useStore((s) => s.view);
  const stack = useStore((s) => s.viewStack);
  const goBackTo = useStore((s) => s.goBackTo);
  const detailInstanceId = useStore((s) => s.detailInstanceId);
  const instances = useStore((s) => s.instances);
  const projectRef = useStore((s) => s.projectRef);
  const discoverKind = useStore((s) => s.discoverKind);

  if (stack.length === 0) return null;

  const instance = instances.find((i) => i.id === detailInstanceId);

  const labelFor = (entry: View) => {
    if (entry === "instance") return instance?.name ?? "Instance";
    if (entry === "project") return projectRef?.title ?? "Project";
    if (entry === "discover") return `Discover ${discoverKind}`;
    return LABELS[entry];
  };

  const iconFor = (entry: View) => {
    if (entry !== "instance") return null;
    const logo = logoSrc(instance?.logo ?? null);
    if (!logo) return null;
    return (
      <img
        src={logo}
        alt=""
        className="size-4 shrink-0 rounded bg-surface-3 object-cover"
        draggable={false}
      />
    );
  };

  const crumbs = [...stack, view];

  return (
    <nav
      aria-label="Breadcrumb"
      data-tauri-drag-region
      className="flex min-w-0 items-center gap-0.5 pl-1 pr-2"
    >
      {crumbs.map((entry, index) => {
        const last = index === crumbs.length - 1;
        return (
          <div
            key={`${entry}-${index}`}
            data-tauri-drag-region
            className="flex min-w-0 items-center gap-0.5"
          >
            {index > 0 && (
              <ChevronRight
                className={cn(
                  "pointer-events-none size-3 shrink-0",
                  immersive ? "text-white/40" : "text-content-faint/60",
                )}
              />
            )}
            <button
              onClick={() => !last && goBackTo(index)}
              disabled={last}
              className={cn(
                "flex min-w-0 items-center gap-1.5 rounded-md px-1.5 py-1 text-xs transition-colors",
                last
                  ? immersive
                    ? "font-medium text-white drop-shadow-[0_1px_2px_rgba(0,0,0,0.9)]"
                    : "font-medium text-content"
                  : immersive
                    ? "text-white/70 drop-shadow-[0_1px_2px_rgba(0,0,0,0.9)] hover:bg-white/15 hover:text-white"
                    : "text-content-muted hover:bg-surface-3 hover:text-content",
              )}
            >
              {iconFor(entry)}
              <span className="max-w-52 truncate capitalize">{labelFor(entry)}</span>
            </button>
          </div>
        );
      })}
    </nav>
  );
}
