import { useEffect, useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import {
  ChevronDown,
  Download,
  Loader2,
  Play,
  Plus,
  Terminal,
  TriangleAlert,
} from "lucide-react";

import { api } from "../lib/api";
import { mediaSrc } from "../lib/media";
import type { JavaStatus, VersionMedia } from "../lib/types";
import { CreateInstanceModal } from "../components/CreateInstanceModal";
import { InstanceSheet } from "../components/InstanceSheet";
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
  done: "Ready",
};

function GridFallback() {
  return (
    <div className="absolute inset-0">
      <div className="absolute inset-0" style={gridStyle} />
      <div className="pointer-events-none absolute -left-24 -top-24 size-72 rounded-full bg-[var(--accent-glow)] blur-3xl transition-colors duration-500" />
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
  const installs = useStore((s) => s.installs);
  const installedIds = useStore((s) => s.installedIds);
  const installInstance = useStore((s) => s.installInstance);
  const launchInstance = useStore((s) => s.launchInstance);
  const openConsole = useStore((s) => s.openConsole);
  const running = useStore((s) => s.running);
  const account = useStore((s) => s.accounts.find((a) => a.active) ?? null);
  const mediaMap = useStore((s) => s.media);
  const loadMedia = useStore((s) => s.loadMedia);
  const setView = useStore((s) => s.setView);
  const selectedId = useStore((s) => s.selectedInstanceId);
  const selectInstance = useStore((s) => s.selectInstance);

  const [modalOpen, setModalOpen] = useState(false);
  const [sheetOpen, setSheetOpen] = useState(false);
  const [java, setJava] = useState<JavaStatus | null>(null);
  const [launchError, setLaunchError] = useState<string | null>(null);

  const selected = instances.find((i) => i.id === selectedId) ?? instances[0];
  const hasInstance = !!selected;
  const media = selected ? (mediaMap[selected.id] ?? null) : null;
  const install = selected ? installs[selected.id] : undefined;
  const installing = !!install;
  const installed = selected ? installedIds.includes(selected.id) : false;

  useEffect(() => {
    instances.forEach((i) => loadMedia(i.id));
  }, [instances, loadMedia]);

  useEffect(() => {
    if (!selected) {
      setJava(null);
      return;
    }
    let live = true;
    setJava(null);
    api
      .getJavaStatus(selected.id)
      .then((s) => live && setJava(s))
      .catch(() => live && setJava(null));
    return () => {
      live = false;
    };
  }, [selected?.id]);

  const percent =
    install && install.total > 0 ? Math.round((install.completed / install.total) * 100) : 0;

  const activeRun = selected
    ? Object.values(running).find(
        (r) => r.instance_id === selected.id && r.state === "running",
      )
    : undefined;

  const onAction = async () => {
    setLaunchError(null);
    if (!hasInstance) return setModalOpen(true);
    if (installing) return;
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
    actionLabel = `${STAGE_LABEL[install.stage] ?? install.stage}${
      install.stage === "downloading" ? ` ${percent}%` : ""
    }`;
    actionIcon = <Loader2 className="size-4 animate-spin" />;
  } else if (activeRun) {
    actionLabel = "CONSOLE";
    actionIcon = <Terminal className="size-4" />;
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

        <div className="pointer-events-none absolute inset-x-0 bottom-0 h-1/2 bg-gradient-to-t from-black/85 via-black/35 to-transparent" />

        {java && !java.ok && (
          <div className="absolute right-5 top-5 inline-flex items-center gap-1.5 rounded-full border border-warn/40 bg-black/60 px-3 py-1.5 text-xs font-medium text-warn backdrop-blur">
            <TriangleAlert className="size-3.5" />
            Java {java.required_major} needed
            {java.found ? ` · found ${java.found.major}` : " · none found"}
          </div>
        )}

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
                    VANILLA
                  </span>
                </div>
                {media?.short_text && (
                  <p className="mt-2.5 line-clamp-2 max-w-xl text-sm leading-relaxed text-white/70">
                    {media.short_text}
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

          <button
            onClick={onAction}
            disabled={installing}
            className="relative flex h-14 shrink-0 items-center justify-center gap-2.5 overflow-hidden rounded-2xl px-8 font-pixel text-sm tracking-wider text-black shadow-xl shadow-[var(--accent-glow)] transition-all duration-500 [background:linear-gradient(to_bottom,var(--accent),var(--accent-deep))] hover:[background:linear-gradient(to_bottom,var(--accent-bright),var(--accent))] active:scale-[0.98] disabled:active:scale-100"
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

      {launchError && (
        <div className="flex shrink-0 items-start gap-2 rounded-xl border border-danger/30 bg-danger/10 px-4 py-2.5 text-sm text-danger">
          <TriangleAlert className="mt-0.5 size-4 shrink-0" />
          <span className="break-words">{launchError}</span>
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
      />
    </div>
  );
}
