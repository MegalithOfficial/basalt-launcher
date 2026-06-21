import { useEffect, useState } from "react";
import { motion } from "motion/react";
import {
  Boxes,
  Check,
  Coffee,
  Download,
  Loader2,
  Play,
  Plus,
  TriangleAlert,
} from "lucide-react";

import { cn } from "../lib/cn";
import { api } from "../lib/api";
import type { JavaStatus } from "../lib/types";
import { CreateInstanceModal } from "../components/CreateInstanceModal";
import { useStore } from "../store";

const gridStyle: React.CSSProperties = {
  backgroundImage: `
    linear-gradient(to right, rgba(255,255,255,0.035) 1px, transparent 1px),
    linear-gradient(to bottom, rgba(255,255,255,0.035) 1px, transparent 1px),
    linear-gradient(to right, rgba(255,255,255,0.05) 1px, transparent 1px),
    linear-gradient(to bottom, rgba(255,255,255,0.05) 1px, transparent 1px)`,
  backgroundSize: "32px 32px, 32px 32px, 128px 128px, 128px 128px",
};

const STAGE_LABEL: Record<string, string> = {
  metadata: "Reading metadata",
  "assets-index": "Fetching asset index",
  downloading: "Downloading files",
  natives: "Extracting natives",
  done: "Ready",
};

export function HomeView() {
  const instances = useStore((s) => s.instances);
  const installs = useStore((s) => s.installs);
  const installedIds = useStore((s) => s.installedIds);
  const installInstance = useStore((s) => s.installInstance);
  const account = useStore((s) => s.accounts.find((a) => a.active) ?? null);
  const setView = useStore((s) => s.setView);

  const [modalOpen, setModalOpen] = useState(false);
  const [selectedId, setSelectedId] = useState<string | null>(instances[0]?.id ?? null);
  const [java, setJava] = useState<JavaStatus | null>(null);

  const selected = instances.find((i) => i.id === selectedId) ?? instances[0];
  const hasInstance = !!selected;
  const install = selected ? installs[selected.id] : undefined;
  const installing = !!install;
  const installed = selected ? installedIds.includes(selected.id) : false;

  useEffect(() => {
    if (!selected) {
      setJava(null);
      return;
    }
    let active = true;
    setJava(null);
    api
      .getJavaStatus(selected.id)
      .then((s) => active && setJava(s))
      .catch(() => active && setJava(null));
    return () => {
      active = false;
    };
  }, [selected?.id]);

  const percent =
    install && install.total > 0 ? Math.round((install.completed / install.total) * 100) : 0;

  const onAction = () => {
    if (!hasInstance) setModalOpen(true);
    else if (installing) return;
    else if (!installed) installInstance(selected.id);
    else if (!account) setView("accounts");
  };

  let actionLabel = "INSTALL";
  let actionIcon = <Download className="size-5" />;
  if (!hasInstance) {
    actionLabel = "CREATE INSTANCE";
    actionIcon = <Plus className="size-5" />;
  } else if (installing) {
    actionLabel = `${STAGE_LABEL[install.stage] ?? install.stage}${
      install.stage === "downloading" ? ` ${percent}%` : ""
    }`;
    actionIcon = <Loader2 className="size-5 animate-spin" />;
  } else if (installed && !account) {
    actionLabel = "SIGN IN TO PLAY";
    actionIcon = <Play className="size-5 fill-black" />;
  } else if (installed) {
    actionLabel = "PLAY";
    actionIcon = <Play className="size-5 fill-black" />;
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-4 p-5">
      <motion.div
        initial={{ opacity: 0, y: 10 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.3, ease: "easeOut" }}
        className="relative flex min-h-0 flex-1 flex-col overflow-hidden rounded-2xl border border-border bg-surface"
      >
        <div className="absolute inset-0" style={gridStyle} />
        <div className="pointer-events-none absolute -left-24 -top-24 size-72 rounded-full bg-lava/10 blur-3xl" />
        <div className="pointer-events-none absolute inset-x-0 bottom-0 h-32 bg-gradient-to-t from-base/70 to-transparent" />

        {hasInstance ? (
          <div className="relative flex min-h-0 flex-1 flex-col">
            <div className="flex items-center justify-between px-6 pb-3 pt-5">
              <div className="flex items-center gap-2 text-[11px] font-semibold uppercase tracking-[0.18em] text-content-faint">
                <Boxes className="size-3.5" />
                Your instances
              </div>
              {java && (
                <div
                  className={cn(
                    "inline-flex items-center gap-1.5 rounded-md border px-2 py-1 text-[11px] font-medium",
                    java.ok
                      ? "border-ok/30 bg-ok/10 text-ok"
                      : "border-warn/30 bg-warn/10 text-warn",
                  )}
                >
                  {java.ok ? <Coffee className="size-3" /> : <TriangleAlert className="size-3" />}
                  Java {java.required_major}
                  {java.found ? ` · found ${java.found.major}` : " · missing"}
                </div>
              )}
            </div>

            <div className="grid min-h-0 flex-1 auto-rows-min grid-cols-[repeat(auto-fill,minmax(180px,1fr))] content-start gap-3 overflow-y-auto px-6 pb-6">
              {instances.map((it) => {
                const active = it.id === selected.id;
                const itInstall = installs[it.id];
                const itPercent =
                  itInstall && itInstall.total > 0
                    ? Math.round((itInstall.completed / itInstall.total) * 100)
                    : 0;
                return (
                  <button
                    key={it.id}
                    onClick={() => setSelectedId(it.id)}
                    className={cn(
                      "group relative overflow-hidden rounded-xl border p-4 text-left transition-all",
                      active
                        ? "border-lava/60 bg-lava/10 ring-1 ring-lava/40"
                        : "border-border bg-base/40 backdrop-blur hover:border-border hover:bg-surface-2/60",
                    )}
                  >
                    {active && !itInstall && (
                      <span className="absolute right-3 top-3 grid size-5 place-items-center rounded-full bg-lava text-black">
                        <Check className="size-3.5" />
                      </span>
                    )}
                    <div className="grid size-9 place-items-center rounded-lg bg-surface-3 text-content-muted">
                      <Boxes className="size-[18px]" />
                    </div>
                    <div className="mt-3 truncate font-display font-semibold text-content">
                      {it.name}
                    </div>
                    <div className="mt-0.5 truncate text-xs text-content-muted">
                      {it.version_id}
                    </div>
                    {itInstall && (
                      <div className="mt-3">
                        <div className="h-1 overflow-hidden rounded-full bg-surface-3">
                          <div
                            className="h-full rounded-full bg-lava transition-all"
                            style={{ width: `${itPercent}%` }}
                          />
                        </div>
                      </div>
                    )}
                  </button>
                );
              })}

              <button
                onClick={() => setModalOpen(true)}
                className="flex min-h-[116px] flex-col items-center justify-center gap-2 rounded-xl border border-dashed border-border bg-transparent text-content-faint transition-colors hover:border-lava/50 hover:text-content-muted"
              >
                <Plus className="size-5" />
                <span className="text-xs font-medium">New instance</span>
              </button>
            </div>
          </div>
        ) : (
          <div className="relative flex min-h-0 flex-1 flex-col items-start justify-start p-8">
            <h1 className="font-pixel text-4xl leading-tight text-content drop-shadow">
              No instance yet
            </h1>
            <p className="mt-4 max-w-md text-sm leading-relaxed text-content-muted">
              Create an instance to pick a Minecraft version and start playing. Mods and modpacks
              come later.
            </p>
          </div>
        )}
      </motion.div>

      <button
        onClick={onAction}
        disabled={installing}
        className="group relative flex h-14 shrink-0 items-center justify-center gap-2.5 overflow-hidden rounded-2xl bg-gradient-to-b from-lava to-lava-deep font-pixel text-base tracking-wider text-black shadow-xl shadow-lava/25 transition-all hover:from-lava-bright hover:to-lava active:scale-[0.99] disabled:active:scale-100"
      >
        {installing && install.stage === "downloading" && (
          <span
            className="absolute inset-y-0 left-0 bg-black/15 transition-all"
            style={{ width: `${percent}%` }}
          />
        )}
        <span className="relative flex items-center gap-2.5">
          {actionIcon}
          {actionLabel}
        </span>
      </button>

      <CreateInstanceModal
        open={modalOpen}
        onClose={() => setModalOpen(false)}
        onCreated={(id) => setSelectedId(id)}
      />
    </div>
  );
}
