import { useEffect, useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { openPath } from "@tauri-apps/plugin-opener";
import {
  Boxes,
  Clock,
  FolderOpen,
  ImageOff,
  ImagePlus,
  Loader2,
  Trash2,
  X,
} from "lucide-react";

import { api } from "../lib/api";
import { LOADERS, loaderLabel } from "../lib/loader";
import { mediaSrc } from "../lib/media";
import { formatPlaytime, relativeTime } from "../lib/time";
import type { Instance } from "../lib/types";
import { useEscape } from "../lib/useEscape";
import { Select } from "./Select";
import { useStore } from "../store";

const VANILLA = "vanilla";

function loaderWarning(
  oldLoader: string | null,
  oldVersion: string | null,
  newLoader: string | null,
  newVersion: string | null,
): { tone: "info" | "warn" | "danger"; message: string } | null {
  if (oldLoader === newLoader && oldVersion === newVersion) return null;
  if (!oldLoader && newLoader) {
    return {
      tone: "info",
      message:
        "The loader will be set up on the next install. Press Install after saving to download it.",
    };
  }
  if (oldLoader && !newLoader) {
    return {
      tone: "danger",
      message:
        "Switching to Vanilla means your installed mods will stop working. They stay in the mods folder but will not load.",
    };
  }
  if (oldLoader && newLoader && oldLoader !== newLoader) {
    return {
      tone: "danger",
      message: `Mods built for ${oldLoader} will not work on ${newLoader}. You will need to replace them with ${newLoader} builds.`,
    };
  }
  return {
    tone: "warn",
    message:
      "Changing the loader version may require mod updates. Some mods pin a specific loader version.",
  };
}

const inputCls =
  "w-full rounded-lg border border-border bg-base px-3 py-2.5 text-sm text-content outline-none transition-colors focus:border-[var(--accent)]";

function Field({ label, hint, children }: { label: string; hint?: string; children: React.ReactNode }) {
  return (
    <label className="block">
      <div className="mb-1.5 flex items-baseline justify-between">
        <span className="text-sm font-medium text-content">{label}</span>
        {hint && <span className="text-xs text-content-faint">{hint}</span>}
      </div>
      {children}
    </label>
  );
}

export function EditInstanceModal({
  instance,
  onClose,
}: {
  instance: Instance | null;
  onClose: () => void;
}) {
  const mediaMap = useStore((s) => s.media);
  const pickBanner = useStore((s) => s.pickBanner);
  const clearBanner = useStore((s) => s.clearBanner);
  const updateInstance = useStore((s) => s.updateInstance);
  const deleteInstance = useStore((s) => s.deleteInstance);

  const [name, setName] = useState("");
  const [minMem, setMinMem] = useState("");
  const [maxMem, setMaxMem] = useState("");
  const [javaPath, setJavaPath] = useState("");
  const [loader, setLoader] = useState<string>(VANILLA);
  const [loaderVersion, setLoaderVersion] = useState<string | null>(null);
  const [loaderVersions, setLoaderVersions] = useState<string[]>([]);
  const [loaderLoading, setLoaderLoading] = useState(false);
  const [gameVersion, setGameVersion] = useState("");
  const [gameVersions, setGameVersions] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!instance) return;
    setName(instance.name);
    setMinMem(instance.min_memory_mb?.toString() ?? "");
    setMaxMem(instance.max_memory_mb?.toString() ?? "");
    setJavaPath(instance.java_path ?? "");
    setLoader(instance.loader ?? VANILLA);
    setLoaderVersion(instance.loader_version ?? null);
    setGameVersion(instance.version_id);
    setError(null);
  }, [instance?.id]);

  useEffect(() => {
    if (!instance) return;
    let live = true;
    api
      .listVersions(true)
      .then((list) => {
        if (!live) return;
        setGameVersions(list.map((v) => v.id));
      })
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [instance?.id]);

  useEffect(() => {
    if (!instance || loader === VANILLA || !gameVersion) {
      setLoaderVersions([]);
      return;
    }
    let live = true;
    setLoaderLoading(true);
    api
      .listLoaderVersions(loader, gameVersion)
      .then((list) => {
        if (!live) return;
        setLoaderVersions(list);
        setLoaderVersion((current) =>
          current && list.includes(current) ? current : (list[0] ?? null),
        );
      })
      .catch(() => live && setLoaderVersions([]))
      .finally(() => live && setLoaderLoading(false));
    return () => {
      live = false;
    };
  }, [loader, gameVersion, instance?.id]);

  useEscape(!!instance, onClose);

  if (!instance) return null;
  const media = mediaMap[instance.id] ?? null;

  const parseMem = (value: string) => {
    const trimmed = value.trim();
    if (!trimmed) return null;
    const parsed = Number(trimmed);
    return Number.isFinite(parsed) && parsed > 0 ? Math.round(parsed) : null;
  };

  const newLoader = loader === VANILLA ? null : loader;
  const newLoaderVersion = loader === VANILLA ? null : loaderVersion;
  const versionChanged = gameVersion !== instance.version_id;
  const warnings: Array<{ tone: "info" | "warn" | "danger"; message: string }> = [];
  if (versionChanged) {
    warnings.push({
      tone: "warn",
      message: `Changing the game version to ${gameVersion} requires a reinstall, mods must match the new version, and worlds opened on it may not load on ${instance.version_id} again.`,
    });
  }
  const loaderChange = loaderWarning(
    instance.loader,
    instance.loader_version,
    newLoader,
    newLoaderVersion,
  );
  if (loaderChange) warnings.push(loaderChange);

  const save = async () => {
    setBusy(true);
    setError(null);
    try {
      await updateInstance(
        instance.id,
        name,
        parseMem(minMem),
        parseMem(maxMem),
        javaPath.trim() || null,
        newLoader,
        newLoaderVersion,
        gameVersion,
      );
      onClose();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const remove = async () => {
    setBusy(true);
    try {
      await deleteInstance(instance.id);
      onClose();
    } finally {
      setBusy(false);
    }
  };

  return (
    <AnimatePresence>
      <motion.div
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        exit={{ opacity: 0 }}
        className="fixed inset-0 z-50 grid place-items-center bg-black/60 p-6 backdrop-blur-sm"
        onClick={onClose}
      >
        <motion.div
          initial={{ opacity: 0, scale: 0.97, y: 8 }}
          animate={{ opacity: 1, scale: 1, y: 0 }}
          exit={{ opacity: 0, scale: 0.97, y: 8 }}
          transition={{ duration: 0.18 }}
          onClick={(e) => e.stopPropagation()}
          className="flex max-h-[85vh] w-full max-w-2xl flex-col overflow-hidden rounded-2xl border border-border bg-surface shadow-2xl"
        >
          <div className="relative h-44 shrink-0">
            {media ? (
              <img
                src={mediaSrc(media)}
                className="h-full w-full object-cover"
                draggable={false}
              />
            ) : (
              <div className="grid h-full w-full place-items-center bg-surface-2 text-content-faint">
                <Boxes className="size-8" />
              </div>
            )}
            <div className="absolute inset-0 bg-gradient-to-t from-black/80 to-transparent" />
            <button
              onClick={onClose}
              className="absolute right-3 top-3 grid size-8 place-items-center rounded-full bg-black/50 text-white/80 backdrop-blur hover:bg-black/70 hover:text-white"
            >
              <X className="size-4" />
            </button>
            <div className="absolute bottom-3 left-4 right-4 flex items-end justify-between gap-3">
              <div className="min-w-0">
                <div className="truncate font-display text-xl font-bold text-white">
                  {instance.name}
                </div>
                <div className="mt-0.5 flex items-center gap-2 text-[11px] text-white/60">
                  <span className="font-pixel">
                    {instance.version_id}
                    {instance.loader && ` · ${loaderLabel(instance)}`}
                  </span>
                  {instance.last_played_at && (
                    <span className="inline-flex items-center gap-1">
                      <Clock className="size-3" />
                      {relativeTime(instance.last_played_at)}
                      {formatPlaytime(instance.playtime_secs) &&
                        ` · ${formatPlaytime(instance.playtime_secs)}`}
                    </span>
                  )}
                </div>
              </div>
              <div className="flex shrink-0 gap-2">
                <button
                  onClick={() => pickBanner(instance.id)}
                  className="inline-flex items-center gap-1.5 rounded-lg bg-black/50 px-3 py-1.5 text-xs font-medium text-white/85 backdrop-blur hover:bg-black/70"
                >
                  <ImagePlus className="size-3.5" />
                  Change banner
                </button>
                {media?.local && (
                  <button
                    onClick={() => clearBanner(instance.id)}
                    className="inline-flex items-center gap-1.5 rounded-lg bg-black/50 px-3 py-1.5 text-xs font-medium text-white/85 backdrop-blur hover:bg-black/70"
                  >
                    <ImageOff className="size-3.5" />
                    Remove
                  </button>
                )}
              </div>
            </div>
          </div>

          <div className="flex min-h-0 flex-1 flex-col gap-5 overflow-y-auto p-6">
            <Field label="Name">
              <input value={name} onChange={(e) => setName(e.target.value)} className={inputCls} />
            </Field>

            <div className="grid grid-cols-2 gap-4">
              <Field label="Minimum memory" hint="MB, empty = default">
                <input
                  type="number"
                  value={minMem}
                  onChange={(e) => setMinMem(e.target.value)}
                  placeholder="default"
                  className={inputCls}
                />
              </Field>
              <Field label="Maximum memory" hint="MB, empty = default">
                <input
                  type="number"
                  value={maxMem}
                  onChange={(e) => setMaxMem(e.target.value)}
                  placeholder="default"
                  className={inputCls}
                />
              </Field>
            </div>

            <Field label="Java path" hint="empty = auto-detect">
              <input
                value={javaPath}
                onChange={(e) => setJavaPath(e.target.value)}
                placeholder="/usr/lib/jvm/java-25-openjdk/bin/java"
                className={inputCls}
              />
            </Field>

            <Field label="Game version" hint="changing requires a reinstall">
              <Select
                value={gameVersion || null}
                options={gameVersions.length > 0 ? gameVersions : [instance.version_id]}
                onChange={setGameVersion}
                placeholder="Pick a version"
              />
            </Field>

            <div className="grid grid-cols-2 gap-4">
              <Field label="Loader">
                <Select
                  value={loader === VANILLA ? "Vanilla" : (LOADERS.find((l) => l.id === loader)?.label ?? loader)}
                  options={["Vanilla", ...LOADERS.map((l) => l.label)]}
                  onChange={(label) => {
                    const picked = LOADERS.find((l) => l.label === label);
                    setLoader(picked?.id ?? VANILLA);
                    if (!picked) setLoaderVersion(null);
                  }}
                />
              </Field>
              <Field label="Loader version">
                {loader === VANILLA ? (
                  <div className="rounded-lg border border-border-soft bg-surface-2 px-3 py-2 text-sm text-content-faint">
                    Not applicable
                  </div>
                ) : loaderLoading ? (
                  <div className="flex items-center gap-2 rounded-lg border border-border-soft bg-surface-2 px-3 py-2 text-sm text-content-muted">
                    <Loader2 className="size-3.5 animate-spin" />
                    Loading
                  </div>
                ) : loaderVersions.length === 0 ? (
                  <div className="rounded-lg border border-warn/30 bg-warn/10 px-3 py-2 text-sm text-warn">
                    No builds for {instance.version_id}
                  </div>
                ) : (
                  <Select
                    value={loaderVersion}
                    options={loaderVersions.slice(0, 100)}
                    onChange={setLoaderVersion}
                    placeholder="Pick a version"
                  />
                )}
              </Field>
            </div>

            {warnings.map((warning) => (
              <div
                key={warning.message}
                className={
                  warning.tone === "danger"
                    ? "rounded-lg border border-danger/30 bg-danger/10 px-3 py-2 text-sm text-danger"
                    : warning.tone === "warn"
                      ? "rounded-lg border border-warn/30 bg-warn/10 px-3 py-2 text-sm text-warn"
                      : "rounded-lg border border-border bg-surface-2 px-3 py-2 text-sm text-content-muted"
                }
              >
                {warning.message}
              </div>
            ))}

            <button
              onClick={() => openPath(instance.dir)}
              className="inline-flex w-fit items-center gap-2 rounded-lg border border-border bg-surface-2 px-3 py-2 text-xs font-medium text-content-muted transition-colors hover:bg-surface-3 hover:text-content"
            >
              <FolderOpen className="size-3.5" />
              Open instance folder
            </button>

            {error && (
              <div className="rounded-lg border border-danger/30 bg-danger/10 px-3 py-2 text-sm text-danger">
                {error}
              </div>
            )}
          </div>

          <div className="flex items-center justify-between border-t border-border-soft px-6 py-4">
            <button
              onClick={remove}
              disabled={busy}
              className="inline-flex items-center gap-1.5 rounded-lg border border-danger/30 bg-danger/10 px-3 py-2 text-xs font-semibold text-danger transition-colors hover:bg-danger/20 disabled:opacity-50"
            >
              <Trash2 className="size-3.5" />
              Delete instance
            </button>
            <div className="flex gap-2">
              <button
                onClick={onClose}
                className="rounded-lg border border-border bg-surface-2 px-4 py-2 text-sm font-medium text-content hover:bg-surface-3"
              >
                Cancel
              </button>
              <button
                onClick={save}
                disabled={busy || !name.trim()}
                className="inline-flex items-center gap-2 rounded-lg px-4 py-2 text-sm font-semibold text-black shadow-lg shadow-[var(--accent-glow)] transition-all [background:linear-gradient(to_bottom,var(--accent),var(--accent-deep))] hover:[background:linear-gradient(to_bottom,var(--accent-bright),var(--accent))] disabled:cursor-not-allowed disabled:opacity-50"
              >
                {busy && <Loader2 className="size-4 animate-spin" />}
                Save
              </button>
            </div>
          </div>
        </motion.div>
      </motion.div>
    </AnimatePresence>
  );
}
