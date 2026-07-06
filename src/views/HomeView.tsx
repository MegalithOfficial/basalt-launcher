import { useEffect, useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import {
  Download,
  Loader2,
  Play,
  Plus,
  Terminal,
  TriangleAlert,
} from "lucide-react";

import { cn } from "../lib/cn";
import { api } from "../lib/api";
import { accentVars } from "../lib/accent";
import type { Instance, JavaStatus, VersionMedia } from "../lib/types";
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
        src={media.image_url}
        onError={() => setFailed(true)}
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        exit={{ opacity: 0 }}
        transition={{ duration: 0.3 }}
        className="absolute inset-0 h-full w-full object-cover"
        draggable={false}
      />
    </AnimatePresence>
  );
}

function ShelfTile({
  instance,
  media,
  active,
  onSelect,
}: {
  instance: Instance;
  media: VersionMedia | null;
  active: boolean;
  onSelect: () => void;
}) {
  return (
    <button
      onClick={onSelect}
      className={cn(
        "group relative h-24 w-40 shrink-0 overflow-hidden rounded-xl border text-left transition-all duration-300",
        active
          ? "border-transparent ring-2 ring-[var(--accent)] shadow-lg shadow-[var(--accent-glow)]"
          : "border-border opacity-70 hover:opacity-100",
      )}
    >
      {media ? (
        <img
          src={media.image_url}
          className="absolute inset-0 h-full w-full object-cover"
          draggable={false}
        />
      ) : (
        <div className="absolute inset-0 bg-surface-2" style={gridStyle} />
      )}
      <div className="absolute inset-x-0 bottom-0 bg-gradient-to-t from-black/85 to-transparent px-3 pb-2 pt-6">
        <div className="truncate text-xs font-semibold text-white">{instance.name}</div>
        <div className="truncate font-pixel text-[9px] text-white/60">
          {instance.version_id}
        </div>
      </div>
    </button>
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

  const [modalOpen, setModalOpen] = useState(false);
  const [selectedId, setSelectedId] = useState<string | null>(instances[0]?.id ?? null);
  const [java, setJava] = useState<JavaStatus | null>(null);
  const [launchError, setLaunchError] = useState<string | null>(null);

  const selected = instances.find((i) => i.id === selectedId) ?? instances[0];
  const hasInstance = !!selected;
  const media = selected ? (mediaMap[selected.version_id] ?? null) : null;
  const install = selected ? installs[selected.id] : undefined;
  const installing = !!install;
  const installed = selected ? installedIds.includes(selected.id) : false;

  useEffect(() => {
    instances.forEach((i) => loadMedia(i.version_id));
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
    <div
      className="flex min-h-0 flex-1 flex-col gap-4 p-5"
      style={accentVars(media?.accent)}
    >
      <div className="relative flex min-h-0 flex-1 flex-col overflow-hidden rounded-3xl border border-border bg-surface">
        {hasInstance ? (
          <HeroArt media={media} id={selected.id} />
        ) : (
          <GridFallback />
        )}

        <div className="pointer-events-none absolute inset-x-0 top-0 h-24 bg-gradient-to-b from-black/50 to-transparent" />
        <div className="pointer-events-none absolute inset-x-0 bottom-0 h-2/3 bg-gradient-to-t from-black/90 via-black/45 to-transparent" />

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
                <h1 className="truncate font-display text-5xl font-bold tracking-tight text-white drop-shadow-lg">
                  {selected.name}
                </h1>
                <div className="mt-3 flex items-center gap-2">
                  <span className="rounded-md bg-black/50 px-2 py-1 font-pixel text-[10px] tracking-wider text-white/80 backdrop-blur">
                    {selected.version_id}
                  </span>
                  <span className="rounded-md bg-black/50 px-2 py-1 font-pixel text-[10px] tracking-wider text-white/50 backdrop-blur">
                    VANILLA
                  </span>
                </div>
                {media?.short_text && (
                  <p className="mt-3 max-w-xl truncate text-sm text-white/70">
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

      <div className="flex shrink-0 items-center gap-3 overflow-x-auto pb-1">
        {instances.map((it) => (
          <ShelfTile
            key={it.id}
            instance={it}
            media={mediaMap[it.version_id] ?? null}
            active={selected?.id === it.id}
            onSelect={() => setSelectedId(it.id)}
          />
        ))}
        <button
          onClick={() => setModalOpen(true)}
          className="flex h-24 w-40 shrink-0 flex-col items-center justify-center gap-1.5 rounded-xl border border-dashed border-border text-content-faint transition-colors hover:border-[var(--accent)] hover:text-content-muted"
        >
          <Plus className="size-5" />
          <span className="text-xs font-medium">New instance</span>
        </button>
      </div>

      <CreateInstanceModal
        open={modalOpen}
        onClose={() => setModalOpen(false)}
        onCreated={(id) => setSelectedId(id)}
      />
    </div>
  );
}
