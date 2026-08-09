import { useCallback, useEffect, useRef, useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  ChevronDown,
  Download,
  Loader2,
  Pencil,
  Play,
  Plus,
  Square,
  TriangleAlert,
} from "lucide-react";

import { api } from "../lib/api";
import { mediaSrc } from "../lib/media";
import { loaderLabel } from "../lib/loader";
import { formatPlaytime, relativeTime } from "../lib/time";
import { useUptime } from "../lib/useUptime";
import type { JavaStatus, VersionMedia } from "../lib/types";
import { CreateInstanceModal } from "../components/CreateInstanceModal";
import { UploadModal } from "../components/UploadModal";
import { ImportPackModal } from "../components/ImportPackModal";
import { InstanceSheet } from "../components/InstanceSheet";
import { instanceTaskLabel, taskFraction, useInstanceTask } from "../lib/useTasks";
import type { PackImportSource } from "../lib/packs";
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
  downloading: "Downloading",
  natives: "Extracting natives",
  "loader-profile": "Fetching loader",
  "loader-installer": "Setting up loader",
  done: "Ready",
};

function GridFallback() {
  return (
    <div className="absolute inset-0">
      <div className="absolute inset-0" style={gridStyle} />
      <div className="pointer-events-none absolute -left-24 -top-24 size-72 rounded-full bg-(--accent-glow) blur-3xl transition-colors duration-500" />
    </div>
  );
}

function HeroArt({ media, id }: { media: VersionMedia | null; id: string }) {
  const [failed, setFailed] = useState(false);

  useEffect(() => setFailed(false), [media?.image_url]);

  if (!media || failed) return <GridFallback />;

  return (
    <AnimatePresence mode="popLayout">
      <motion.img
        key={id + media.image_url}
        src={mediaSrc(media)}
        onError={() => setFailed(true)}
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        exit={{ opacity: 0 }}
        transition={{ duration: 0.4 }}
        className={
          media.local
            ? "absolute inset-0 h-full w-full object-cover"
            : "absolute inset-0 h-full w-full object-cover [image-rendering:pixelated]"
        }
        draggable={false}
      />
    </AnimatePresence>
  );
}

export function HomeView() {
  const instances = useStore((s) => s.instances);
  const installedIds = useStore((s) => s.installedIds);
  const installInstance = useStore((s) => s.installInstance);
  const launchInstance = useStore((s) => s.launchInstance);
  const launching = useStore((s) => s.launching);
  const killInstance = useStore((s) => s.killInstance);
  const openConsole = useStore((s) => s.openConsole);
  const openInstance = useStore((s) => s.openInstance);
  const running = useStore((s) => s.running);
  const account = useStore((s) => s.accounts.find((a) => a.active) ?? null);
  const mediaMap = useStore((s) => s.media);
  const loadMedia = useStore((s) => s.loadMedia);
  const setView = useStore((s) => s.setView);
  const selectedId = useStore((s) => s.selectedInstanceId);
  const selectInstance = useStore((s) => s.selectInstance);

  const [modalOpen, setModalOpen] = useState(false);
  const [packSource, setPackSource] = useState<PackImportSource | null>(null);
  const [picking, setPicking] = useState(false);
  const [sheetOpen, setSheetOpen] = useState(false);
  const [java, setJava] = useState<JavaStatus | null>(null);
  const [installingJava, setInstallingJava] = useState(false);
  const [launchError, setLaunchError] = useState<string | null>(null);
  const javaRequest = useRef(0);

  const selected = instances.find((i) => i.id === selectedId) ?? instances[0];
  const hasInstance = !!selected;
  const media = selected ? (mediaMap[selected.id] ?? null) : null;
  const install = useInstanceTask(selected?.id);
  const installing = !!install;
  const installed = selected ? installedIds.includes(selected.id) : false;

  useEffect(() => {
    instances.forEach((i) => loadMedia(i.id));
  }, [instances, loadMedia]);

  const refreshJava = useCallback(async () => {
    const request = ++javaRequest.current;
    if (!selected) {
      setJava(null);
      return;
    }
    setJava(null);
    try {
      const status = await api.getJavaStatus(selected.id);
      if (request === javaRequest.current) setJava(status);
    } catch {
      if (request === javaRequest.current) setJava(null);
    }
  }, [selected?.id]);

  useEffect(() => {
    void refreshJava();
  }, [refreshJava]);

  useEffect(() => {
    const onFocus = () => void refreshJava();
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [refreshJava]);

  const downloadJava = async () => {
    if (!java || !selected || installingJava) return;
    setInstallingJava(true);
    setLaunchError(null);
    try {
      const installed = await api.installJavaRuntime(java.required_major, selected.id);
      useStore.setState((state) => ({
        instances: state.instances.map((instance) =>
          instance.id === selected.id ? { ...instance, java_path: installed.path } : instance,
        ),
      }));
      setJava({ required_major: java.required_major, found: installed, ok: true });
    } catch (error) {
      setLaunchError(String(error));
    } finally {
      setInstallingJava(false);
    }
  };

  const percent = install ? Math.round((taskFraction(install) ?? 0) * 100) : 0;

  const activeRun = selected
    ? Object.values(running).find(
        (r) => r.instance_id === selected.id && r.state === "running",
      )
    : undefined;

  const starting = selected ? launching.includes(selected.id) : false;
  const uptime = useUptime(activeRun?.started_at ?? 0, !!activeRun);

  const onAction = async () => {
    setLaunchError(null);
    if (!hasInstance) return setModalOpen(true);
    if (installing || installingJava || starting) return;
    if (activeRun) return openConsole(activeRun.running_id);
    if (!installed) return installInstance(selected.id);
    if (!account) return setView("accounts");
    try {
      await launchInstance(selected.id);
    } catch (e) {
      setLaunchError(String(e));
    }
  };

  let actionLabel = "INSTALL";
  let actionIcon = <Download className="size-4" />;
  if (!hasInstance) {
    actionLabel = "CREATE INSTANCE";
    actionIcon = <Plus className="size-4" />;
  } else if (installing) {
    const taskLabel =
      install.kind === "instance_repair" ||
      install.kind === "instance_duplicate" ||
      install.kind === "modpack_upgrade" ||
      install.kind === "snapshot_create" ||
      install.kind === "snapshot_restore"
        ? instanceTaskLabel(install)
        : (STAGE_LABEL[install.stage] ?? instanceTaskLabel(install));
    const showProgress =
      install.stage === "downloading" ||
      install.kind === "instance_repair" ||
      install.kind === "instance_duplicate" ||
      install.kind === "modpack_upgrade" ||
      install.kind === "snapshot_create" ||
      install.kind === "snapshot_restore";
    actionLabel = `${taskLabel}${
      showProgress && taskFraction(install) != null ? ` ${percent}%` : ""
    }`;
    actionIcon = <Loader2 className="size-4 animate-spin" />;
  } else if (starting) {
    actionLabel = "STARTING";
    actionIcon = <Loader2 className="size-4 animate-spin" />;
  } else if (activeRun) {
    actionLabel = `RUNNING ${uptime.toUpperCase()}`;
    actionIcon = (
      <span className="relative flex size-2">
        <span className="absolute inline-flex size-full animate-ping rounded-full bg-black/60" />
        <span className="relative inline-flex size-2 rounded-full bg-black/80" />
      </span>
    );
  } else if (installed && !account) {
    actionLabel = "SIGN IN";
    actionIcon = <Play className="size-4 fill-black" />;
  } else if (installed) {
    actionLabel = "PLAY";
    actionIcon = <Play className="size-4 fill-black" />;
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3 p-5">
      <div className="relative flex min-h-0 flex-1 flex-col overflow-hidden rounded-3xl border border-border bg-surface">
        {hasInstance ? (
          <HeroArt media={media} id={selected.id} />
        ) : (
          <GridFallback />
        )}

        <div className="pointer-events-none absolute inset-x-0 bottom-0 h-1/2 bg-linear-to-t from-black/85 via-black/35 to-transparent" />

        <div className="absolute right-5 top-5 flex items-center gap-2">
          {java && !java.ok && (
            <div className="pointer-events-auto flex items-center gap-2 rounded-xl border border-warn/40 bg-black/75 p-2 pl-3 text-xs font-medium text-warn shadow-xl backdrop-blur">
              <TriangleAlert className="size-4 shrink-0" />
              <span className="mr-1">
                Java {java.required_major} needed
                {java.found ? ` · found ${java.found.major}` : " · none found"}
              </span>
              <button
                onClick={() => void downloadJava()}
                disabled={installingJava}
                className="inline-flex items-center gap-1.5 rounded-lg bg-warn px-2.5 py-1.5 font-semibold text-black transition-opacity hover:opacity-90 disabled:opacity-60"
              >
                {installingJava ? (
                  <Loader2 className="size-3.5 animate-spin" />
                ) : (
                  <Download className="size-3.5" />
                )}
                {installingJava ? "Downloading" : "Download Java"}
              </button>
              <button
                onClick={() =>
                  void openUrl(
                    `https://adoptium.net/temurin/releases/?version=${java.required_major}`,
                  )
                }
                className="rounded-lg border border-white/15 px-2.5 py-1.5 text-white/75 transition-colors hover:bg-white/10 hover:text-white"
              >
                Install manually
              </button>
            </div>
          )}
          {hasInstance && (
            <button
              onClick={() => openInstance(selected.id)}
              aria-label="Manage instance"
              className="grid size-9 place-items-center rounded-full border border-white/10 bg-black/50 text-white/70 backdrop-blur transition-colors hover:bg-black/70 hover:text-white"
            >
              <Pencil className="size-4" />
            </button>
          )}
        </div>

        <div className="absolute inset-x-0 bottom-0 flex items-end justify-between gap-6 p-8">
          <div className="min-w-0">
            {hasInstance ? (
              <>
                <button
                  onClick={() => setSheetOpen(true)}
                  className="group flex max-w-full items-center gap-2.5 text-left"
                >
                  <h1 className="truncate font-display text-4xl font-bold tracking-tight text-white drop-shadow-lg transition-colors group-hover:text-white/80">
                    {selected.name}
                  </h1>
                  <span className="mt-1.5 grid size-7 shrink-0 place-items-center rounded-full bg-black/40 text-white/60 backdrop-blur transition-all group-hover:bg-black/60 group-hover:text-white">
                    <ChevronDown className="size-4 transition-transform group-hover:translate-y-0.5" />
                  </span>
                </button>
                <div className="mt-2.5 flex items-center gap-2">
                  {selected.name !== selected.version_id && (
                    <span className="rounded-md bg-black/50 px-2 py-1 font-pixel text-[10px] tracking-wider text-white/80 backdrop-blur">
                      {selected.version_id}
                    </span>
                  )}
                  <span className="rounded-md bg-black/50 px-2 py-1 font-pixel text-[10px] tracking-wider text-white/50 backdrop-blur">
                    {loaderLabel(selected).toUpperCase()}
                  </span>
                  {selected.pack_project_id && (
                    <span className="rounded-md bg-(--accent-glow) px-2 py-1 font-pixel text-[10px] tracking-wider text-white/80 backdrop-blur">
                      MODPACK
                    </span>
                  )}
                </div>
                {media?.short_text && (
                  <p className="mt-2.5 line-clamp-2 max-w-xl text-sm leading-relaxed text-white/70">
                    {media.short_text}
                  </p>
                )}
                {selected.last_played_at && (
                  <p className="mt-2 text-xs text-white/50">
                    Last played {relativeTime(selected.last_played_at)}
                    {formatPlaytime(selected.playtime_secs) &&
                      ` · ${formatPlaytime(selected.playtime_secs)}`}
                  </p>
                )}
              </>
            ) : (
              <>
                <h1 className="font-display text-4xl font-bold tracking-tight text-content">
                  No instance yet
                </h1>
                <p className="mt-2 max-w-md text-sm text-content-muted">
                  Create an instance to pick a Minecraft version and start playing.
                </p>
              </>
            )}
          </div>

          <div className="flex shrink-0 items-center gap-2">
            {activeRun && (
              <button
                onClick={() => killInstance(activeRun.running_id)}
                title="Stop the game"
                className="grid h-14 w-14 shrink-0 place-items-center rounded-2xl border border-white/15 bg-black/50 text-white/80 backdrop-blur transition-colors hover:border-danger/50 hover:bg-danger/20 hover:text-danger"
              >
                <Square className="size-4 fill-current" />
              </button>
            )}
            <button
              onClick={onAction}
              disabled={installing || installingJava || starting}
              className="relative flex h-14 shrink-0 items-center justify-center gap-2.5 overflow-hidden rounded-2xl px-8 font-pixel text-sm tracking-wider text-black shadow-xl shadow-(color:--accent-glow) transition-all duration-500 [background:linear-gradient(to_bottom,var(--accent),var(--accent-deep))] hover:[background:linear-gradient(to_bottom,var(--accent-bright),var(--accent))] active:scale-[0.98] disabled:active:scale-100"
            >
              {installing && install.stage === "downloading" && (
                <span
                  className="absolute inset-y-0 left-0 bg-black/20 transition-all"
                  style={{ width: `${percent}%` }}
                />
              )}
              <span className="relative flex items-center gap-2.5">
                {actionIcon}
                {actionLabel}
              </span>
            </button>

          </div>
        </div>
      </div>

      {launchError && (
        <div className="flex shrink-0 items-start gap-2 rounded-xl border border-danger/30 bg-danger/10 px-4 py-2.5 text-sm text-danger">
          <TriangleAlert className="mt-0.5 size-4 shrink-0" />
          <span className="wrap-break-word">{launchError}</span>
        </div>
      )}

      <InstanceSheet
        open={sheetOpen}
        onClose={() => setSheetOpen(false)}
        onCreate={() => setModalOpen(true)}
      />

      <CreateInstanceModal
        open={modalOpen}
        onClose={() => setModalOpen(false)}
        onCreated={(id) => selectInstance(id)}
        onImportFile={() => setPicking(true)}
        onImportPackwiz={setPackSource}
      />
      <UploadModal
        open={picking}
        onClose={() => setPicking(false)}
        title="Import a modpack"
        subtitle="An .mrpack, CurseForge zip, or local pack.toml"
        extensions={["mrpack", "zip", "toml"]}
        filterName="Modpack"
        confirmLabel="Import"
        onConfirm={(paths) => {
          setPicking(false);
          setPackSource({ kind: "file", value: paths[0] });
        }}
      />
      <ImportPackModal
        source={packSource}
        onClose={() => setPackSource(null)}
        onImported={(instance) => selectInstance(instance.id)}
      />
    </div>
  );
}
