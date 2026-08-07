import { useEffect, useState } from "react";
import { openPath } from "@tauri-apps/plugin-opener";
import { motion } from "motion/react";
import {
  Boxes,
  FolderOpen,
  ImageOff,
  ImagePlus,
  Link2,
  Loader2,
  Lock,
  Package,
  Plus,
  Trash2,
  Unlink,
  X,
} from "lucide-react";

import { api } from "../lib/api";
import { cn } from "../lib/cn";
import { LOADERS, loaderLabel } from "../lib/loader";
import { logoSrc } from "../lib/media";
import { formatPlaytime, relativeTime } from "../lib/time";
import type { EnvVar, Instance, JavaInfo, ProjectSummary, SystemStats } from "../lib/types";
import { Banner } from "./Banner";
import { BannerLibraryModal } from "./BannerLibraryModal";
import { ConfirmDialog } from "./ConfirmDialog";
import { LinkModpackModal } from "./LinkModpackModal";
import { MemoryRange } from "./MemoryRange";
import { Modal } from "./Modal";
import { Select } from "./Select";
import { SettingGroup, SettingRow } from "./ui";
import { toast } from "sonner";

import { useStore } from "../store";

const VANILLA = "vanilla";
const JAVA_AUTO = "Auto-detect";
const JAVA_CUSTOM = "Custom path";

const APPEND = "Append to defaults";
const REPLACE = "Replace defaults";
const MODES = [APPEND, REPLACE];

function parseEnv(text: string | null): EnvVar[] {
  return (text ?? "")
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => {
      const at = line.indexOf("=");
      return at === -1
        ? { key: line, value: "" }
        : { key: line.slice(0, at).trim(), value: line.slice(at + 1).trim() };
    });
}

function serializeEnv(entries: EnvVar[]): string {
  return entries
    .filter((entry) => entry.key.trim())
    .map((entry) => `${entry.key.trim()}=${entry.value}`)
    .join("\n");
}

function Locked({ value }: { value: string }) {
  return (
    <div
      title="The modpack decides this"
      className="flex items-center gap-2 rounded-lg border border-border-soft bg-surface-3 px-3 py-2 text-sm text-content-muted"
    >
      <Lock className="size-3.5 shrink-0 text-content-faint" />
      <span className="truncate">{value}</span>
    </div>
  );
}

interface Draft {
  name: string;
  minMem: string;
  maxMem: string;
  javaPath: string;
  javaCustom: boolean;
  loader: string;
  loaderVersion: string | null;
  gameVersion: string;
  jvmArgs: string;
  jvmArgsMode: string;
  envVars: EnvVar[];
  envVarsMode: string;
  notes: string;
  groupId: string | null;
  wrapper: string;
  preLaunch: string;
  postExit: string;
}

function draftFrom(instance: Instance): Draft {
  return {
    name: instance.name,
    minMem: instance.min_memory_mb?.toString() ?? "",
    maxMem: instance.max_memory_mb?.toString() ?? "",
    javaPath: instance.java_path ?? "",
    javaCustom: false,
    loader: instance.loader ?? VANILLA,
    loaderVersion: instance.loader_version ?? null,
    gameVersion: instance.version_id,
    jvmArgs: instance.jvm_args ?? "",
    jvmArgsMode: instance.jvm_args_mode ?? "append",
    envVars: parseEnv(instance.env_vars),
    envVarsMode: instance.env_vars_mode ?? "append",
    notes: instance.notes ?? "",
    groupId: null,
    wrapper: instance.wrapper_command ?? "",
    preLaunch: instance.pre_launch_command ?? "",
    postExit: instance.post_exit_command ?? "",
  };
}

type Tab = "general" | "appearance" | "installation" | "java";

const TABS: Array<{ id: Tab; label: string }> = [
  { id: "general", label: "General" },
  { id: "appearance", label: "Appearance" },
  { id: "installation", label: "Installation" },
  { id: "java", label: "Java" },
];

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
  "rounded-lg border border-border bg-void px-3 py-2 text-sm text-content outline-none transition-colors focus:border-(--accent)";

const chipCls =
  "inline-flex items-center gap-1.5 rounded-lg border border-border bg-surface-2 px-3 py-2 text-xs font-medium text-content-muted transition-colors hover:bg-surface-3 hover:text-content";

export function EditInstanceModal({
  instance,
  onClose,
}: {
  instance: Instance | null;
  onClose: () => void;
}) {
  const mediaMap = useStore((s) => s.media);
  const clearBanner = useStore((s) => s.clearBanner);
  const clearLogo = useStore((s) => s.clearLogo);
  const applyBanner = useStore((s) => s.applyBanner);
  const applyLogo = useStore((s) => s.applyLogo);
  const updateInstance = useStore((s) => s.updateInstance);
  const deleteInstance = useStore((s) => s.deleteInstance);
  const organization = useStore((s) => s.instanceOrganization);
  const moveInstanceToGroup = useStore((s) => s.moveInstanceToGroup);
  const refreshInstances = useStore((s) => s.refreshInstances);

  const [tab, setTab] = useState<Tab>("general");
  const [draft, setDraft] = useState<Draft | null>(null);
  const [draftOf, setDraftOf] = useState<string | null>(null);
  const [loaderVersions, setLoaderVersions] = useState<string[]>([]);
  const [loaderLoading, setLoaderLoading] = useState(false);
  const [gameVersions, setGameVersions] = useState<string[]>([]);
  const [javas, setJavas] = useState<JavaInfo[]>([]);
  const [stats, setStats] = useState<SystemStats | null>(null);
  const [busy, setBusy] = useState(false);
  const [confirmRemove, setConfirmRemove] = useState(false);
  const [library, setLibrary] = useState<"banner" | "logo" | null>(null);
  const [pack, setPack] = useState<ProjectSummary | null>(null);
  const [packVersion, setPackVersion] = useState<string | null>(null);
  const [unlinking, setUnlinking] = useState(false);
  const [linking, setLinking] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const openFor = instance?.id ?? null;
  if (draftOf !== openFor) {
    setDraftOf(openFor);
    if (instance) {
      setDraft(draftFrom(instance));
      setTab("general");
      setError(null);
    }
  }

  const set = (patch: Partial<Draft>) =>
    setDraft((current) => (current ? { ...current, ...patch } : current));

  const loader = draft?.loader ?? VANILLA;
  const gameVersion = draft?.gameVersion ?? "";

  useEffect(() => {
    const provider = instance?.pack_provider;
    const project = instance?.pack_project_id;
    const version = instance?.pack_version_id;
    if (!provider || !project) {
      setPack(null);
      setPackVersion(null);
      return;
    }
    let live = true;
    setPack(null);
    setPackVersion(null);
    api
      .resolveProjects(provider, [project])
      .then((list) => live && setPack(list[0] ?? null))
      .catch(() => {});
    if (version) {
      api
        .listProjectVersions(provider, project, "modpacks", "", null)
        .then((list) => {
          const found = list.find((entry) => entry.id === version);
          if (live) setPackVersion(found?.version_number ?? found?.name ?? null);
        })
        .catch(() => {});
    }
    return () => {
      live = false;
    };
  }, [instance?.pack_provider, instance?.pack_project_id, instance?.pack_version_id]);

  useEffect(() => {
    if (!instance) return;
    api.listJavas().then(setJavas).catch(() => {});
    api.getSystemStats().then(setStats).catch(() => {});
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
        setDraft((current) =>
          current && current.loaderVersion && list.includes(current.loaderVersion)
            ? current
            : current && { ...current, loaderVersion: list[0] ?? null },
        );
      })
      .catch(() => live && setLoaderVersions([]))
      .finally(() => live && setLoaderLoading(false));
    return () => {
      live = false;
    };
  }, [loader, gameVersion, instance?.id]);

  if (!instance || !draft) return null;
  const { name, minMem, maxMem, javaPath, javaCustom, loaderVersion, jvmArgs, jvmArgsMode, envVars, envVarsMode, notes } = draft;
  const placedIn =
    draft.groupId !== null
      ? draft.groupId
      : (organization.placements.find((p) => p.instance_id === instance.id)?.group_id ?? "");
  const groupLabels = ["No group", ...organization.groups.map((group) => group.name)];
  const groupLabel =
    organization.groups.find((group) => group.id === placedIn)?.name ?? "No group";
  const packLocked = !!instance.pack_project_id;
  const media = mediaMap[instance.id] ?? null;
  const logo = logoSrc(instance.logo);

  const memoryCeiling = Math.max(4096, stats?.total_memory_mb ?? 16384);
  const usingDefaults = !minMem.trim() && !maxMem.trim();
  const sliderMin = Number(minMem) > 0 ? Number(minMem) : 512;
  const sliderMax = Number(maxMem) > 0 ? Number(maxMem) : 4096;

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
        jvmArgs.trim() || null,
        jvmArgsMode,
        serializeEnv(envVars) || null,
        envVarsMode,
      );
      await api.setInstanceNotes(instance.id, notes);
      await api.setInstanceLaunchTools(
        instance.id,
        draft.wrapper,
        draft.preLaunch,
        draft.postExit,
      );
      const current =
        organization.placements.find((p) => p.instance_id === instance.id)?.group_id ?? null;
      if (draft.groupId !== null && draft.groupId !== current) {
        await moveInstanceToGroup(instance.id, draft.groupId || null);
      }
      await refreshInstances();
      onClose();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const remove = async () => {
    await deleteInstance(instance.id);
    setConfirmRemove(false);
    onClose();
  };

  return (
    <Modal open onClose={onClose} size="xl" className="h-[min(680px,calc(100vh-48px))]">
      <div className="relative h-24 shrink-0 overflow-hidden">
        {media ? (
          <Banner media={media} className="h-full w-full" />
        ) : (
          <div className="h-full w-full bg-surface-2" />
        )}
        <div className="absolute inset-0 bg-linear-to-t from-surface via-surface/85 to-surface/40" />

        <div className="absolute inset-x-5 bottom-3 flex items-end gap-3">
          {logo ? (
            <img
              src={logo}
              alt=""
              className="size-11 shrink-0 rounded-xl bg-surface-3 object-cover ring-1 ring-white/10"
              draggable={false}
            />
          ) : (
            <span className="grid size-11 shrink-0 place-items-center rounded-xl bg-surface-3 text-content-faint ring-1 ring-white/10">
              <Boxes className="size-5" />
            </span>
          )}
          <div className="min-w-0 flex-1">
            <div className="truncate font-display text-lg font-semibold text-content">
              {instance.name}
            </div>
            <div className="mt-0.5 flex flex-wrap items-center gap-x-2 text-[11px] text-content-faint">
              <span className="font-pixel">{instance.version_id}</span>
              <span>·</span>
              <span>{loaderLabel(instance)}</span>
              {instance.last_played_at && (
                <>
                  <span>·</span>
                  <span>played {relativeTime(instance.last_played_at)}</span>
                </>
              )}
              {formatPlaytime(instance.playtime_secs) && (
                <>
                  <span>·</span>
                  <span>{formatPlaytime(instance.playtime_secs)}</span>
                </>
              )}
            </div>
          </div>
        </div>

        <button
          onClick={onClose}
          aria-label="Close"
          className="absolute right-3 top-3 grid size-8 place-items-center rounded-lg bg-black/40 text-white/80 backdrop-blur transition-colors hover:bg-black/70 hover:text-white"
        >
          <X className="size-4" />
        </button>
      </div>

      <div
        role="tablist"
        aria-label="Instance settings"
        className="flex shrink-0 items-center gap-6 border-b border-border-soft px-5"
      >
        {TABS.map((entry) => (
          <button
            key={entry.id}
            role="tab"
            aria-selected={tab === entry.id}
            onClick={() => setTab(entry.id)}
            className={cn(
              "relative -mb-px pb-3 pt-2.5 text-sm font-medium transition-colors",
              tab === entry.id
                ? "text-content"
                : "text-content-faint hover:text-content-muted",
            )}
          >
            {entry.label}
            {tab === entry.id && (
              <motion.span
                layoutId="instance-settings-underline"
                transition={{ type: "spring", stiffness: 500, damping: 40 }}
                className="absolute inset-x-0 -bottom-px h-0.5 rounded-full bg-(--accent)"
              />
            )}
          </button>
        ))}
      </div>

      <div className="flex min-h-0 flex-1 flex-col gap-5 overflow-y-auto p-5">
        {tab === "general" && (
          <>
            <SettingGroup>
              <SettingRow label="Name" hint="Shown across the launcher">
                <input
                  value={name}
                  onChange={(e) => set({ name: e.target.value })}
                  className={cn(inputCls, "w-64")}
                />
              </SettingRow>
              <SettingRow label="Group" hint="Where this instance sits on the instances page">
                <div className="w-64">
                  <Select
                    value={groupLabel}
                    options={groupLabels}
                    onChange={(label) =>
                      set({
                        groupId:
                          organization.groups.find((group) => group.name === label)?.id ?? "",
                      })
                    }
                  />
                </div>
              </SettingRow>
              <SettingRow label="Instance folder" hint={instance.dir} stacked>
                <button onClick={() => openPath(instance.dir)} className={chipCls}>
                  <FolderOpen className="size-3.5" />
                  Open folder
                </button>
              </SettingRow>
            </SettingGroup>

            <SettingGroup
              title="Notes"
              description="Only you see this. Server addresses, what the world is for, what not to update."
            >
              <div className="p-4">
                <textarea
                  value={notes}
                  onChange={(e) => set({ notes: e.target.value })}
                  rows={6}
                  placeholder="Nothing noted yet"
                  className="selectable w-full resize-y rounded-lg border border-border bg-void px-3 py-2 text-sm leading-relaxed text-content outline-none transition-colors placeholder:text-content-faint focus:border-(--accent)"
                />
              </div>
            </SettingGroup>
          </>
        )}

        {tab === "appearance" && (
          <>
            <SettingGroup title="Banner" description="Fills the hero on the play and instance pages.">
              <div className="p-4">
                <div className="h-32 overflow-hidden rounded-xl border border-border-soft">
                  {media ? (
                    <Banner media={media} className="h-full w-full" />
                  ) : (
                    <div className="grid h-full w-full place-items-center bg-surface-3 text-content-faint">
                      <Boxes className="size-6" />
                    </div>
                  )}
                </div>
                <div className="mt-3 flex flex-wrap gap-2">
                  <button onClick={() => setLibrary("banner")} className={chipCls}>
                    <ImagePlus className="size-3.5" />
                    Choose banner
                  </button>
                  {media?.local && (
                    <button onClick={() => clearBanner(instance.id)} className={chipCls}>
                      <ImageOff className="size-3.5" />
                      Remove banner
                    </button>
                  )}
                </div>
              </div>
            </SettingGroup>

            <SettingGroup title="Logo" description="Used in the sidebar dock and lists.">
              <div className="flex items-center gap-4 p-4">
                {logo ? (
                  <img
                    src={logo}
                    alt=""
                    className="size-16 shrink-0 rounded-xl bg-surface-3 object-cover"
                    draggable={false}
                  />
                ) : (
                  <span className="grid size-16 shrink-0 place-items-center rounded-xl bg-surface-3 text-content-faint">
                    <Boxes className="size-6" />
                  </span>
                )}
                <div className="flex flex-wrap gap-2">
                  <button onClick={() => setLibrary("logo")} className={chipCls}>
                    <ImagePlus className="size-3.5" />
                    {instance.logo ? "Change logo" : "Add logo"}
                  </button>
                  {instance.logo && (
                    <button onClick={() => clearLogo(instance.id)} className={chipCls}>
                      <ImageOff className="size-3.5" />
                      Remove logo
                    </button>
                  )}
                </div>
              </div>
            </SettingGroup>
          </>
        )}

        {tab === "installation" && (
          <>
            {!instance.pack_project_id && (
              <SettingGroup
                title="Modpack"
                description="Follow a pack so this instance can receive its updates."
              >
                <SettingRow
                  label="Not following a modpack"
                  hint="Link one if this instance came from a pack, and Basalt will offer its updates"
                  stacked
                >
                  <button onClick={() => setLinking(true)} className={chipCls}>
                    <Link2 className="size-3.5" />
                    Link a modpack
                  </button>
                </SettingRow>
              </SettingGroup>
            )}

            {instance.pack_project_id && (
              <SettingGroup
                title="Modpack"
                description="Where this instance came from and what it follows for updates."
              >
                <div className="flex items-center gap-3 p-4">
                  <span className="grid size-10 shrink-0 place-items-center rounded-xl bg-surface-3 text-content-faint">
                    <Package className="size-5" />
                  </span>
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-sm font-medium text-content">
                      {pack?.title ?? "Reading the pack"}
                    </div>
                    <div className="truncate text-[11px] text-content-faint">
                      {instance.pack_provider === "modrinth" ? "Modrinth" : "CurseForge"}
                      {packVersion ? ` · ${packVersion}` : ""}
                    </div>
                  </div>
                  <button
                    onClick={() => setUnlinking(true)}
                    className={chipCls}
                    title="Stop following this modpack"
                  >
                    <Unlink className="size-3.5" />
                    Unlink
                  </button>
                </div>
              </SettingGroup>
            )}

            <SettingGroup
              title="Version and loader"
              description={
                packLocked
                  ? "The modpack decides these. Unlink it above to set them yourself."
                  : "Changing either one needs a reinstall before the next launch."
              }
            >
              <SettingRow label="Game version">
                <div className="w-56">
                  {packLocked ? (
                    <Locked value={instance.version_id} />
                  ) : (
                    <Select
                      value={gameVersion || null}
                      options={gameVersions.length > 0 ? gameVersions : [instance.version_id]}
                      onChange={(value) => set({ gameVersion: value })}
                      placeholder="Pick a version"
                    />
                  )}
                </div>
              </SettingRow>
              <SettingRow label="Loader">
                <div className="w-56">
                  {packLocked ? (
                    <Locked
                      value={
                        instance.loader
                          ? (LOADERS.find((l) => l.id === instance.loader)?.label ??
                            instance.loader)
                          : "Vanilla"
                      }
                    />
                  ) : (
                    <Select
                      value={
                        loader === VANILLA
                          ? "Vanilla"
                          : (LOADERS.find((l) => l.id === loader)?.label ?? loader)
                      }
                      options={["Vanilla", ...LOADERS.map((l) => l.label)]}
                      onChange={(label) => {
                        const picked = LOADERS.find((l) => l.label === label);
                        set({ loader: picked?.id ?? VANILLA });
                        if (!picked) set({ loaderVersion: null });
                      }}
                    />
                  )}
                </div>
              </SettingRow>
              <SettingRow label="Loader version">
                <div className="w-56">
                  {packLocked ? (
                    <Locked value={instance.loader_version ?? "Not applicable"} />
                  ) : loader === VANILLA ? (
                    <div className="rounded-lg border border-border-soft bg-surface-3 px-3 py-2 text-sm text-content-faint">
                      Not applicable
                    </div>
                  ) : loaderLoading ? (
                    <div className="flex items-center gap-2 rounded-lg border border-border-soft bg-surface-3 px-3 py-2 text-sm text-content-muted">
                      <Loader2 className="size-3.5 animate-spin" />
                      Loading
                    </div>
                  ) : loaderVersions.length === 0 ? (
                    <div className="rounded-lg border border-warn/30 bg-warn/10 px-3 py-2 text-sm text-warn">
                      No builds
                    </div>
                  ) : (
                    <Select
                      value={loaderVersion}
                      options={loaderVersions.slice(0, 100)}
                      onChange={(value) => set({ loaderVersion: value })}
                      placeholder="Pick a version"
                    />
                  )}
                </div>
              </SettingRow>
            </SettingGroup>

            {warnings.map((warning) => (
              <div
                key={warning.message}
                className={cn(
                  "rounded-xl border px-3.5 py-3 text-xs leading-relaxed",
                  warning.tone === "danger"
                    ? "border-danger/30 bg-danger/10 text-danger"
                    : warning.tone === "warn"
                      ? "border-warn/30 bg-warn/10 text-warn"
                      : "border-border bg-surface-2 text-content-muted",
                )}
              >
                {warning.message}
              </div>
            ))}
          </>
        )}

        {tab === "java" && (
          <>
            <SettingGroup
              title="Memory"
              description="Leave both empty to follow the launcher defaults."
            >
              <div className="px-5 py-5">
                <div className="mb-2 flex items-end justify-between gap-4">
                  <div>
                    <div className="text-[11px] font-medium text-content-muted">Minimum</div>
                    <div className="mt-1 flex items-center gap-1.5">
                      <input
                        type="number"
                        value={minMem}
                        onChange={(e) => set({ minMem: e.target.value })}
                        placeholder="default"
                        className={cn(inputCls, "w-24 text-right tabular-nums")}
                      />
                      <span className="text-xs text-content-faint">MB</span>
                    </div>
                  </div>

                  <div className="text-right">
                    <div className="text-[11px] font-medium text-content-muted">Maximum</div>
                    <div className="mt-1 flex items-center gap-1.5">
                      <input
                        type="number"
                        value={maxMem}
                        onChange={(e) => set({ maxMem: e.target.value })}
                        placeholder="default"
                        className={cn(inputCls, "w-24 text-right tabular-nums")}
                      />
                      <span className="text-xs text-content-faint">MB</span>
                    </div>
                  </div>
                </div>

                <div className="px-1 pt-3">
                  <MemoryRange
                    min={sliderMin}
                    max={sliderMax}
                    ceiling={memoryCeiling}
                    available={stats?.available_memory_mb}
                    onChange={(low, high) => {
                      set({ minMem: String(low), maxMem: String(high) });
                      
                    }}
                  />
                </div>

                <div className="mt-4 flex items-center justify-between gap-4 border-t border-border-soft pt-3 text-[11px]">
                  <span
                    className={cn(
                      stats && sliderMax > stats.available_memory_mb && !usingDefaults
                        ? "text-warn"
                        : "text-content-faint",
                    )}
                  >
                    {usingDefaults
                      ? "Following the launcher defaults"
                      : stats && sliderMax > stats.available_memory_mb
                        ? `More than the ${(stats.available_memory_mb / 1024).toFixed(1)} GB free right now`
                        : `Starts at ${(sliderMin / 1024).toFixed(1)} GB, never above ${(sliderMax / 1024).toFixed(1)} GB`}
                  </span>
                  {!usingDefaults && (
                    <button
                      onClick={() => {
                        set({ minMem: "", maxMem: "" });
                        
                      }}
                      className="shrink-0 font-medium text-content-muted transition-colors hover:text-content"
                    >
                      Use defaults
                    </button>
                  )}
                </div>
              </div>
            </SettingGroup>

            <SettingGroup title="Java">
              <SettingRow label="Runtime" hint={`${javas.length} detected on this system`}>
                <div className="w-64">
                  <Select
                    value={
                      javaCustom
                        ? JAVA_CUSTOM
                        : !javaPath
                          ? JAVA_AUTO
                          : javas.find((j) => j.path === javaPath)
                            ? `Java ${javas.find((j) => j.path === javaPath)!.major} · ${javaPath}`
                            : JAVA_CUSTOM
                    }
                    options={[
                      JAVA_AUTO,
                      ...javas.map((j) => `Java ${j.major} · ${j.path}`),
                      JAVA_CUSTOM,
                    ]}
                    onChange={(choice) => {
                      if (choice === JAVA_AUTO) {
                        set({ javaCustom: false, javaPath: "" });
                        return;
                      }
                      if (choice === JAVA_CUSTOM) {
                        set({ javaCustom: true });
                        return;
                      }
                      const picked = javas.find(
                        (j) => `Java ${j.major} · ${j.path}` === choice,
                      );
                      if (picked) {
                        set({ javaCustom: false, javaPath: picked.path });
                      }
                    }}
                  />
                </div>
              </SettingRow>
              {(javaCustom || (!!javaPath && !javas.some((j) => j.path === javaPath))) && (
                <SettingRow label="Custom path" hint="path to a java executable" stacked>
                  <input
                    value={javaPath}
                    onChange={(e) => set({ javaPath: e.target.value })}
                    placeholder="/path/to/bin/java"
                    className={cn(inputCls, "w-full")}
                  />
                </SettingRow>
              )}
            </SettingGroup>

            <SettingGroup title="Java arguments">
              <SettingRow
                label="Arguments"
                hint={
                  jvmArgsMode === "replace"
                    ? "Used on their own, ignoring the launcher defaults"
                    : "Added after the launcher defaults"
                }
                stacked
                action={
                  <div className="w-48">
                    <Select
                      compact
                      value={jvmArgsMode === "replace" ? REPLACE : APPEND}
                      options={MODES}
                      onChange={(choice) =>
                        set({ jvmArgsMode: choice === REPLACE ? "replace" : "append" })
                      }
                    />
                  </div>
                }
              >
                <textarea
                  value={jvmArgs}
                  onChange={(e) => set({ jvmArgs: e.target.value })}
                  rows={2}
                  spellCheck={false}
                  placeholder="-XX:+UseG1GC -Dsome.flag=true"
                  className={cn(inputCls, "w-full resize-y font-mono text-xs")}
                />
              </SettingRow>
            </SettingGroup>

            <SettingGroup
              title="Around the launch"
              description="Leave a box empty to follow the launcher setting."
            >
              <SettingRow label="Wrapper command" hint="mangohud, gamemoderun, and the like" stacked>
                <input
                  value={draft.wrapper}
                  onChange={(e) => set({ wrapper: e.target.value })}
                  spellCheck={false}
                  placeholder="follows the launcher setting"
                  className={cn(inputCls, "w-full font-mono text-xs")}
                />
              </SettingRow>
              <SettingRow
                label="Before launching"
                hint="the launch stops if this fails"
                stacked
              >
                <input
                  value={draft.preLaunch}
                  onChange={(e) => set({ preLaunch: e.target.value })}
                  spellCheck={false}
                  placeholder="follows the launcher setting"
                  className={cn(inputCls, "w-full font-mono text-xs")}
                />
              </SettingRow>
              <SettingRow label="After the game exits" hint="failures only reach the log" stacked>
                <input
                  value={draft.postExit}
                  onChange={(e) => set({ postExit: e.target.value })}
                  spellCheck={false}
                  placeholder="follows the launcher setting"
                  className={cn(inputCls, "w-full font-mono text-xs")}
                />
              </SettingRow>
            </SettingGroup>

            <SettingGroup
              title="Environment variables"
              description="Set on the game process only. Useful for driver and GPU switches."
            >
              <SettingRow
                label="When launching"
                hint={
                  envVarsMode === "replace"
                    ? "Only these are set, the launcher defaults are ignored"
                    : "Merged over the launcher defaults, matching names win"
                }
              >
                <div className="w-48">
                  <Select
                    compact
                    value={envVarsMode === "replace" ? REPLACE : APPEND}
                    options={MODES}
                    onChange={(choice) =>
                      set({ envVarsMode: choice === REPLACE ? "replace" : "append" })
                    }
                  />
                </div>
              </SettingRow>

              <div className="flex flex-col gap-2 px-5 py-4">
                {envVars.length === 0 && (
                  <p className="text-xs text-content-faint">Nothing set for this instance.</p>
                )}
                {envVars.map((entry, index) => (
                  <div key={index} className="flex items-center gap-2">
                    <input
                      value={entry.key}
                      onChange={(e) =>
                        set({
                          envVars: envVars.map((v, i) =>
                            i === index ? { ...v, key: e.target.value } : v,
                          ),
                        })
                      }
                      placeholder="MESA_GL_VERSION_OVERRIDE"
                      spellCheck={false}
                      className={cn(inputCls, "min-w-0 flex-1 font-mono text-xs")}
                    />
                    <span className="text-xs text-content-faint">=</span>
                    <input
                      value={entry.value}
                      onChange={(e) =>
                        set({
                          envVars: envVars.map((v, i) =>
                            i === index ? { ...v, value: e.target.value } : v,
                          ),
                        })
                      }
                      placeholder="4.5"
                      spellCheck={false}
                      className={cn(inputCls, "min-w-0 flex-1 font-mono text-xs")}
                    />
                    <button
                      onClick={() =>
                        set({ envVars: envVars.filter((_, i) => i !== index) })
                      }
                      title={`Remove ${entry.key || "variable"}`}
                      className="grid size-8 shrink-0 place-items-center rounded-lg text-content-faint transition-colors hover:bg-danger/15 hover:text-danger"
                    >
                      <Trash2 className="size-4" />
                    </button>
                  </div>
                ))}
                <button
                  onClick={() => set({ envVars: [...envVars, { key: "", value: "" }] })}
                  className={cn(chipCls, "mt-1 self-start")}
                >
                  <Plus className="size-3.5" />
                  Add variable
                </button>
              </div>
            </SettingGroup>
          </>
        )}

        {error && (
          <div className="rounded-xl border border-danger/30 bg-danger/10 px-3.5 py-3 text-sm text-danger">
            {error}
          </div>
        )}
      </div>

      <div className="flex items-center justify-between border-t border-border-soft px-5 py-3.5">
        <button
          onClick={() => setConfirmRemove(true)}
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
            className="inline-flex items-center gap-2 rounded-lg px-4 py-2 text-sm font-semibold text-black shadow-lg shadow-(color:--accent-glow) transition-all [background:linear-gradient(to_bottom,var(--accent),var(--accent-deep))] hover:[background:linear-gradient(to_bottom,var(--accent-bright),var(--accent))] disabled:cursor-not-allowed disabled:opacity-50"
          >
            {busy && <Loader2 className="size-4 animate-spin" />}
            Save
          </button>
        </div>
      </div>

      <BannerLibraryModal
        open={library !== null}
        mode={library ?? "banner"}
        currentId={library === "banner" ? instance.banner_id : null}
        onClose={() => setLibrary(null)}
        onPick={async (entry) => {
          if (library === "logo") await applyLogo(instance.id, entry.id);
          else await applyBanner(instance.id, entry.id);
        }}
      />

      <LinkModpackModal
        instance={instance}
        open={linking}
        onClose={() => setLinking(false)}
      />

      <ConfirmDialog
        open={unlinking}
        nested
        tone="warn"
        title={`Stop following ${pack?.title ?? "this modpack"}?`}
        description="Its mods and configs stay exactly where they are, but they become yours to manage. Basalt will no longer offer pack updates for this instance."
        confirmLabel="Unlink"
        onConfirm={async () => {
          try {
            await api.unlinkModpack(instance.id);
            await useStore.getState().refreshInstances();
            toast.success(`${instance.name} is no longer following a pack`, {
              description: "Its content now updates like any other instance.",
            });
          } catch (error) {
            toast.error("Could not unlink the modpack", { description: String(error) });
          }
          setUnlinking(false);
        }}
        onCancel={() => setUnlinking(false)}
      />

      <ConfirmDialog
        open={confirmRemove}
        nested
        title={`Delete ${instance.name}?`}
        confirmLabel="Delete instance"
        requireText={instance.name}
        description="The whole instance folder is removed from disk, including its worlds, mods, configs and screenshots. This cannot be undone."
        onConfirm={remove}
        onCancel={() => setConfirmRemove(false)}
      />
    </Modal>
  );
}
