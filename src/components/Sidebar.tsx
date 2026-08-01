import { useEffect, useMemo, useRef, useState } from "react";
import { openPath } from "@tauri-apps/plugin-opener";
import { motion } from "motion/react";
import { toast } from "sonner";
import {
  Boxes,
  CircleStop,
  Compass,
  Copy,
  Download,
  FolderOpen,
  Pin,
  PinOff,
  Play,
  Settings,
  Share,
  SquareChartGantt,
  Trash2,
  UserCircle2,
  Wrench,
} from "lucide-react";

import { cn } from "../lib/cn";
import { logoSrc, mediaSrc } from "../lib/media";
import type { Instance, VersionMedia, View } from "../lib/types";
import { PlayerHead } from "./Avatar";
import { ConfirmDialog } from "./ConfirmDialog";
import { ContextMenu, useContextMenu, type MenuItem } from "./ContextMenu";
import { EditInstanceModal } from "./EditInstanceModal";
import { ExportPackModal } from "./ExportPackModal";
import { useStore } from "../store";

const MAX_TILES = 5;
const TILE_SIZE = 44;
const TILE_GAP = 6;
const DOCK_HEADING = 25;
const PIN_KEY = "sidebar-pins";

function readPins(): string[] {
  try {
    const raw = localStorage.getItem(PIN_KEY);
    const parsed = raw ? (JSON.parse(raw) as unknown) : null;
    return Array.isArray(parsed) ? parsed.filter((v): v is string => typeof v === "string") : [];
  } catch {
    return [];
  }
}

const NAV: Array<{ id: View; label: string; icon: typeof Play }> = [
  { id: "home", label: "Play", icon: Play },
  { id: "instances", label: "Instances", icon: Boxes },
  { id: "discover", label: "Discover", icon: Compass },
];

function RailLabel({ children }: { children: React.ReactNode }) {
  return (
    <span className="pointer-events-none absolute left-[calc(100%+12px)] top-1/2 z-50 max-w-44 -translate-x-1 -translate-y-1/2 truncate whitespace-nowrap rounded-lg border border-border bg-surface-3 px-2 py-1 text-[11px] font-medium text-content opacity-0 shadow-xl transition duration-150 group-hover:translate-x-0 group-hover:opacity-100">
      {children}
    </span>
  );
}

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
    <div className="relative flex w-full justify-center">
      {active && (
        <motion.span
          layoutId="rail-active"
          className="absolute left-0 top-1/2 h-7 w-0.75 -translate-y-1/2 rounded-r-full bg-(--accent) shadow-[0_0_10px_var(--accent-glow)]"
          transition={{ type: "spring", stiffness: 520, damping: 40 }}
        />
      )}
      <button
        onClick={onClick}
        disabled={disabled}
        aria-label={label}
        aria-current={active ? "page" : undefined}
        className={cn(
          "group relative grid size-12 place-items-center rounded-xl outline-none transition-colors duration-150 focus-visible:ring-2 focus-visible:ring-(--accent)",
          disabled
            ? "cursor-not-allowed text-content-faint/40"
            : active
              ? "bg-[color-mix(in_srgb,var(--accent)_15%,transparent)] text-(--accent-bright)"
              : "text-content-faint hover:bg-surface-2 hover:text-content",
        )}
      >
        {children}
        <RailLabel>{label}</RailLabel>
      </button>
    </div>
  );
}

function RecentTile({
  instance,
  media,
  active,
  running,
  pinned,
  onClick,
  onContextMenu,
}: {
  instance: Instance;
  media: VersionMedia | null;
  active: boolean;
  running: boolean;
  pinned: boolean;
  onClick: () => void;
  onContextMenu: (e: React.MouseEvent) => void;
}) {
  const [broken, setBroken] = useState<string[]>([]);
  const logo = logoSrc(instance.logo);
  const banner = media ? mediaSrc(media) : null;
  const src = [logo, banner].find((c): c is string => !!c && !broken.includes(c)) ?? null;

  useEffect(() => setBroken([]), [instance.logo, media?.image_url]);

  return (
    <div className="relative flex w-full justify-center">
      {active && (
        <motion.span
          layoutId="rail-active"
          className="absolute left-0 top-1/2 h-7 w-0.75 -translate-y-1/2 rounded-r-full bg-(--accent) shadow-[0_0_10px_var(--accent-glow)]"
          transition={{ type: "spring", stiffness: 520, damping: 40 }}
        />
      )}
      <button
        onClick={onClick}
        onContextMenu={onContextMenu}
        aria-label={instance.name}
        aria-current={active ? "page" : undefined}
        className="group relative grid size-11 place-items-center rounded-xl outline-none transition-transform duration-150 hover:scale-105 active:scale-95 focus-visible:ring-2 focus-visible:ring-(--accent)"
      >
        <span className="absolute inset-0 grid place-items-center overflow-hidden rounded-[10px] bg-surface-2">
          {src ? (
            <img
              src={src}
              onError={() => setBroken((b) => (b.includes(src) ? b : [...b, src]))}
              alt=""
              draggable={false}
              className={cn(
                "size-full object-cover",
                src === banner && media && !media.local && "[image-rendering:pixelated]",
              )}
            />
          ) : (
            <span className="font-display text-[13px] font-semibold text-content-muted">
              {instance.name.slice(0, 1).toUpperCase()}
            </span>
          )}
        </span>
        <span
          className={cn(
            "pointer-events-none absolute inset-0 rounded-[10px] ring-inset transition-colors",
            active ? "ring-2 ring-(--accent)" : running ? "ring-2 ring-ok" : "ring-1 ring-white/10",
          )}
        />
        {pinned && (
          <span className="pointer-events-none absolute -right-1 -top-1 grid size-3.75 place-items-center rounded-full bg-surface-3 text-content-muted ring-2 ring-base">
            <Pin className="size-2.5" />
          </span>
        )}
        <RailLabel>{instance.name}</RailLabel>
      </button>
    </div>
  );
}

export function Sidebar() {
  const view = useStore((s) => s.view);
  const setView = useStore((s) => s.setView);
  const activeAccount = useStore((s) => s.accounts.find((a) => a.active));
  const running = useStore((s) => s.running);
  const openConsole = useStore((s) => s.openConsole);
  const instances = useStore((s) => s.instances);
  const mediaMap = useStore((s) => s.media);
  const detailInstanceId = useStore((s) => s.detailInstanceId);
  const openInstance = useStore((s) => s.openInstance);
  const loadMedia = useStore((s) => s.loadMedia);
  const installedIds = useStore((s) => s.installedIds);
  const installInstance = useStore((s) => s.installInstance);
  const launchInstance = useStore((s) => s.launchInstance);
  const openDiscover = useStore((s) => s.openDiscover);
  const killInstance = useStore((s) => s.killInstance);
  const deleteInstance = useStore((s) => s.deleteInstance);
  const duplicateInstance = useStore((s) => s.duplicateInstance);
  const repairInstance = useStore((s) => s.repairInstance);

  const { menu, open: openMenu, close: closeMenu } = useContextMenu();
  const dockRef = useRef<HTMLDivElement>(null);
  const [capacity, setCapacity] = useState(MAX_TILES);

  useEffect(() => {
    const node = dockRef.current;
    if (!node) return;
    const measure = () => {
      const room = node.clientHeight - DOCK_HEADING;
      setCapacity(Math.max(0, Math.floor((room + TILE_GAP) / (TILE_SIZE + TILE_GAP))));
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(node);
    return () => observer.disconnect();
  }, []);
  const [pins, setPins] = useState<string[]>(readPins);
  const [editing, setEditing] = useState<Instance | null>(null);
  const [exporting, setExporting] = useState<Instance | null>(null);
  const [removing, setRemoving] = useState<Instance | null>(null);

  const anyRunning = Object.values(running).some((r) => r.state === "running");
  const liveRuns = Object.values(running).filter((r) => r.state === "running");
  const runningIds = new Set(liveRuns.map((r) => r.instance_id));

  const togglePin = (id: string) =>
    setPins((current) => {
      const next = current.includes(id)
        ? current.filter((v) => v !== id)
        : [...current, id];
      localStorage.setItem(PIN_KEY, JSON.stringify(next));
      return next;
    });

  const dock = useMemo(() => {
    const byId = new Map(instances.map((i) => [i.id, i]));
    const pinned = pins.map((id) => byId.get(id)).filter((i): i is Instance => !!i);
    const pinnedIds = new Set(pinned.map((i) => i.id));
    const recents = instances
      .filter((i) => i.last_played_at && !pinnedIds.has(i.id))
      .sort((a, b) => (b.last_played_at ?? 0) - (a.last_played_at ?? 0))
      .slice(0, Math.max(0, MAX_TILES - pinned.length));
    return { pinned, recents };
  }, [instances, pins]);

  const tiles = [...dock.pinned, ...dock.recents];
  const shownTiles = tiles.slice(0, capacity);

  useEffect(() => {
    tiles.forEach((i) => void loadMedia(i.id));
  }, [instances, pins, loadMedia]);

  const tileMenu = (instance: Instance): MenuItem[] => {
    const isRunning = runningIds.has(instance.id);
    const run = liveRuns.find((r) => r.instance_id === instance.id);
    const installed = installedIds.includes(instance.id);
    const pinned = pins.includes(instance.id);

    return [
      installed
        ? {
            label: isRunning ? "Running" : "Play",
            icon: Play,
            disabled: isRunning,
            onSelect: () => {
              launchInstance(instance.id).catch((e) =>
                toast.error(`Could not launch ${instance.name}`, { description: String(e) }),
              );
            },
          }
        : {
            label: "Install",
            icon: Download,
            onSelect: () => {
              installInstance(instance.id).catch((e) =>
                toast.error(`Could not install ${instance.name}`, { description: String(e) }),
              );
            },
          },
      ...(run
        ? [
            {
              label: "Stop",
              icon: CircleStop,
              onSelect: () => {
                killInstance(run.running_id).catch((e) =>
                  toast.error(`Could not stop ${instance.name}`, { description: String(e) }),
                );
              },
            },
            {
              label: "Live logs",
              icon: SquareChartGantt,
              onSelect: () => openConsole(run.running_id),
            },
          ]
        : []),
      {
        label: "Open instance",
        icon: Boxes,
        separated: true,
        onSelect: () => openInstance(instance.id),
      },
      {
        label: instance.loader ? "Find mods" : "Find mods (requires loader)",
        icon: Compass,
        disabled: !instance.loader,
        onSelect: () => openDiscover("mods", instance.id),
      },
      {
        label: "Open folder",
        icon: FolderOpen,
        onSelect: () => void openPath(instance.dir),
      },
      {
        label: "Repair and verify",
        icon: Wrench,
        disabled: isRunning,
        onSelect: () => {
          repairInstance(instance.id)
            .then((report) => {
              if (report.unresolved.length > 0) {
                toast.warning(
                  `Repair finished with ${report.unresolved.length} unresolved ${report.unresolved.length === 1 ? "file" : "files"}`,
                  { description: report.unresolved.slice(0, 3).join(" · ") },
                );
              } else {
                toast.success(`${instance.name} is healthy`, {
                  description:
                    report.repaired_content > 0
                      ? `Repaired ${report.repaired_content} tracked ${report.repaired_content === 1 ? "file" : "files"}.`
                      : "Game and tracked content files passed verification.",
                });
              }
            })
            .catch((error) => {
              if (!/cancelled/i.test(String(error))) {
                toast.error(`Could not repair ${instance.name}`, {
                  description: String(error),
                });
              }
            });
        },
      },
      {
        label: "Export as pack",
        icon: Share,
        onSelect: () => setExporting(instance),
      },
      {
        label: "Duplicate instance",
        icon: Copy,
        disabled: isRunning,
        onSelect: () => {
          duplicateInstance(instance.id).catch((error) =>
            toast.error(`Could not duplicate ${instance.name}`, {
              description: String(error),
            }),
          );
        },
      },
      {
        label: "Settings",
        icon: Settings,
        separated: true,
        onSelect: () => setEditing(instance),
      },
      {
        label: pinned ? "Unpin from sidebar" : "Pin to sidebar",
        icon: pinned ? PinOff : Pin,
        onSelect: () => togglePin(instance.id),
      },
      {
        label: "Delete instance",
        icon: Trash2,
        danger: true,
        separated: true,
        onSelect: () => setRemoving(instance),
      },
    ];
  };

  return (
    <aside className="relative z-30 flex w-18.5 shrink-0 flex-col items-center border-r border-border-soft bg-surface/40 py-3">
      <button
        onClick={() => setView("home")}
        aria-label="Basalt"
        className="group relative size-10 shrink-0 overflow-hidden rounded-xl transition-transform hover:scale-105 active:scale-95"
      >
        <img
          src="/logo.png"
          alt="Basalt"
          className="size-full object-contain"
          draggable={false}
        />
      </button>

      <div className="my-3 h-px w-8 shrink-0 bg-border-soft" />

      <nav className="flex w-full flex-col items-center gap-1">
        {NAV.map(({ id, label, icon: Icon }) => (
          <RailButton key={id} label={label} active={view === id} onClick={() => setView(id)}>
            <Icon className="size-5.25" />
          </RailButton>
        ))}
      </nav>

      <div ref={dockRef} className="flex min-h-0 w-full flex-1 flex-col items-center overflow-hidden">
        {shownTiles.length > 0 && (
          <>
            <div className="my-3 h-px w-8 shrink-0 bg-border-soft" />
            <div className="flex w-full flex-col items-center gap-1.5">
              {shownTiles.map((instance) => (
              <RecentTile
                key={instance.id}
                instance={instance}
                media={mediaMap[instance.id] ?? null}
                active={view === "instance" && detailInstanceId === instance.id}
                running={runningIds.has(instance.id)}
                pinned={pins.includes(instance.id)}
                onClick={() => openInstance(instance.id)}
                onContextMenu={(e) =>
                  openMenu(e, tileMenu(instance), instance.name, { fromElement: true })
                }
                />
              ))}
            </div>
          </>
        )}
      </div>

      <div className="my-3 h-px w-8 shrink-0 bg-border-soft" />

      <div className="flex w-full flex-col items-center gap-1">
        <RailButton
          label={anyRunning ? "Logs (running)" : "Logs"}
          active={view === "logs"}
          onClick={() => setView("logs")}
        >
          <span className="relative">
            <SquareChartGantt className="size-5.25" />
            {anyRunning && view !== "logs" && (
              <span className="absolute -right-1 -top-1 size-2 rounded-full bg-ok ring-2 ring-base" />
            )}
          </span>
        </RailButton>

        <RailButton
          label="Settings"
          active={view === "settings"}
          onClick={() => setView("settings")}
        >
          <Settings className="size-5.25" />
        </RailButton>

        <RailButton
          label={activeAccount ? activeAccount.name : "Sign in"}
          active={view === "accounts"}
          onClick={() => setView("accounts")}
        >
          {activeAccount ? (
            <PlayerHead uuid={activeAccount.id} name={activeAccount.name} size={30} />
          ) : (
            <UserCircle2 className="size-5.25" />
          )}
        </RailButton>
      </div>

      <ContextMenu menu={menu} onClose={closeMenu} />

      <EditInstanceModal instance={editing} onClose={() => setEditing(null)} />
      <ExportPackModal instance={exporting} onClose={() => setExporting(null)} />

      <ConfirmDialog
        open={!!removing}
        title={removing ? `Delete ${removing.name}?` : ""}
        description="The whole instance folder is removed from disk, including its worlds, mods, configs and screenshots. This cannot be undone."
        confirmLabel="Delete instance"
        requireText={removing?.name}
        onConfirm={async () => {
          if (removing) await deleteInstance(removing.id);
          setRemoving(null);
        }}
        onCancel={() => setRemoving(null)}
      />
    </aside>
  );
}
