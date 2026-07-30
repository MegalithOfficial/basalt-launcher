import { useEffect, useRef, useState } from "react";
import { motion } from "motion/react";
import { openPath, openUrl } from "@tauri-apps/plugin-opener";
import {
  ArrowUpCircle,
  Bug,
  Check,
  CircleCheck,
  FolderOpen,
  HardDriveDownload,
  KeyRound,
  Plus,
  RefreshCw,
  ScrollText,
  Tag,
  Trash2,
} from "lucide-react";

import { MemoryRange } from "../components/MemoryRange";
import { MigrateModal } from "../components/MigrateModal";
import { Select } from "../components/Select";
import { SettingGroup, SettingRow as Row, Toggle } from "../components/ui";
import { api } from "../lib/api";
import { cn } from "../lib/cn";
import { log } from "../lib/log";
import type {
  AboutLinks,
  AppInfo,
  JavaInfo,
  EnvVar,
  LaunchPreview,
  LauncherSettings,
  LogLevel,
  SystemStats,
  SystemUsage,
  UpdateInfo,
} from "../lib/types";
import { useStore } from "../store";

const TABS = [
  { id: "general", label: "General" },
  { id: "java", label: "Java" },
  { id: "game", label: "Game" },
  { id: "integrations", label: "Integrations" },
  { id: "resources", label: "Resources" },
];

function Section(props: React.ComponentProps<typeof SettingGroup>) {
  return <SettingGroup {...props} className={cn("mb-6", props.className)} />;
}

const AUTO_DETECT = "Auto-detect";
const CUSTOM_PATH = "Custom path";

const inputCls =
  "rounded-lg border border-border bg-base px-3 py-2 text-sm text-content outline-none transition-colors placeholder:text-content-faint focus:border-[var(--accent)]";

const numberCls =
  "w-24 text-right [appearance:textfield] [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none";

const chipCls =
  "inline-flex items-center gap-1.5 rounded-md border border-border bg-surface-2/80 px-2.5 py-1 text-xs font-medium text-content-muted";

const actionCls =
  "inline-flex shrink-0 items-center gap-2 rounded-lg border border-border bg-surface-2 px-3 py-2 text-xs font-medium text-content-muted transition-colors hover:bg-surface-3 hover:text-content";

function GithubMark({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden className={className}>
      <path d="M12 .5A11.5 11.5 0 0 0 .5 12a11.5 11.5 0 0 0 7.86 10.92c.58.1.79-.25.79-.56v-1.98c-3.2.7-3.88-1.38-3.88-1.38-.53-1.34-1.29-1.7-1.29-1.7-1.05-.72.08-.7.08-.7 1.16.08 1.77 1.2 1.77 1.2 1.03 1.77 2.7 1.26 3.36.96.1-.75.4-1.26.73-1.55-2.55-.29-5.24-1.28-5.24-5.7 0-1.26.45-2.29 1.19-3.1-.12-.29-.52-1.46.11-3.05 0 0 .97-.31 3.18 1.18a11 11 0 0 1 5.79 0c2.2-1.49 3.17-1.18 3.17-1.18.63 1.59.23 2.76.12 3.05.74.81 1.18 1.84 1.18 3.1 0 4.43-2.69 5.4-5.25 5.69.41.36.78 1.06.78 2.14v3.17c0 .31.2.67.8.56A11.5 11.5 0 0 0 23.5 12 11.5 11.5 0 0 0 12 .5Z" />
    </svg>
  );
}

function formatGb(mb?: number | null) {
  if (mb == null) return "unknown";
  if (mb < 1024) return `${mb} MB`;
  return `${(mb / 1024).toFixed(1)} GB`;
}

function StatTile({
  label,
  value,
  hint,
  children,
}: {
  label: string;
  value: string;
  hint?: string;
  children?: React.ReactNode;
}) {
  return (
    <div className="flex min-w-0 flex-col gap-1 bg-surface-2/60 px-5 py-4">
      <div className="text-[11px] font-semibold uppercase tracking-wider text-content-faint">
        {label}
      </div>
      <div className="truncate text-sm font-medium text-content" title={value}>
        {value}
      </div>
      {hint && (
        <div className="truncate text-xs text-content-faint" title={hint}>
          {hint}
        </div>
      )}
      {children}
    </div>
  );
}

function SystemCard({
  stats,
  onRefresh,
  refreshing,
}: {
  stats: SystemStats | null;
  onRefresh: () => void;
  refreshing: boolean;
}) {
  return (
    <section className="mt-6">
      <div className="mb-2 flex items-end justify-between gap-4 px-1">
        <div>
          <h2 className="font-display text-sm font-semibold text-content">This system</h2>
          <p className="mt-0.5 text-xs text-content-muted">
            What Basalt sees. Useful when deciding how much memory to hand the game.
          </p>
        </div>
        <button
          onClick={onRefresh}
          disabled={refreshing}
          title="Re-read memory and disk space"
          className="inline-flex shrink-0 items-center gap-1.5 rounded-lg border border-border bg-surface-2 px-2.5 py-1.5 text-[11px] font-medium text-content-muted transition-colors hover:bg-surface-3 hover:text-content disabled:opacity-50"
        >
          <RefreshCw className={cn("size-3.5", refreshing && "animate-spin")} />
          Refresh
        </button>
      </div>
      <div className="grid gap-px overflow-hidden rounded-2xl border border-border-soft bg-border-soft sm:grid-cols-2 xl:grid-cols-4">
        <StatTile
          label="Memory"
          value={`${formatGb(stats?.total_memory_mb)} installed`}
          hint={`${formatGb(stats?.available_memory_mb)} free right now`}
        />
        <StatTile
          label="Processor"
          value={stats?.cpu ?? "reading"}
          hint={stats ? `${stats.cores} physical cores` : undefined}
        />
        <StatTile
          label="Operating system"
          value={stats?.os ?? "reading"}
          hint={stats?.kernel ? `kernel ${stats.kernel}` : undefined}
        />
        <StatTile
          label="Disk"
          value={
            stats?.data_dir_free_mb != null
              ? `${formatGb(stats.data_dir_free_mb)} free`
              : "unknown"
          }
          hint={
            stats?.data_dir_total_mb != null
              ? `of ${formatGb(stats.data_dir_total_mb)}, where instances live`
              : "where instances live"
          }
        />
      </div>
    </section>
  );
}

export function SettingsView() {
  const [migrateOpen, setMigrateOpen] = useState(false);
  const settings = useStore((s) => s.settings);
  const logConfig = useStore((s) => s.logConfig);
  const setLogLevel = useStore((s) => s.setLogLevel);
  const setView = useStore((s) => s.setView);
  const [draft, setDraft] = useState<LauncherSettings | null>(settings);
  const [javas, setJavas] = useState<JavaInfo[]>([]);
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const [saved, setSaved] = useState(false);
  const [stats, setStats] = useState<SystemStats | null>(null);
  const [preview, setPreview] = useState<LaunchPreview | null>(null);
  const [refreshingUsage, setRefreshingUsage] = useState(false);
  const [tab, setTab] = useState(TABS[0].id);
  const [links, setLinks] = useState<AboutLinks | null>(null);
  const [update, setUpdate] = useState<UpdateInfo | null>(null);
  const [checking, setChecking] = useState(false);
  const [checkFailed, setCheckFailed] = useState(false);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const firstRun = useRef(true);

  useEffect(() => setDraft(settings), [settings]);

  useEffect(() => {
    api.listJavas().then((list) => setJavas(list ?? [])).catch(() => {});
    api.getAppInfo().then(setAppInfo).catch(() => {});
    api.getAboutLinks().then(setLinks).catch(() => {});
    api.getSystemStats().then(setStats).catch(() => {});
  }, []);

  useEffect(() => {
    if (!draft) return;
    let live = true;
    api
      .previewLaunchArgs(draft)
      .then((next) => live && setPreview(next))
      .catch(() => live && setPreview(null));
    return () => {
      live = false;
    };
  }, [draft]);

  useEffect(() => {
    if (!draft) return;
    if (firstRun.current) {
      firstRun.current = false;
      return;
    }
    clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(async () => {
      try {
        await api.updateSettings(draft);
        useStore.setState({ settings: draft });
        setSaved(true);
        setTimeout(() => setSaved(false), 1600);
      } catch {
        return;
      }
    }, 500);
    return () => clearTimeout(debounceRef.current);
  }, [draft]);

  if (!draft) return null;

  const set = (patch: Partial<LauncherSettings>) => setDraft({ ...draft, ...patch });

  const javaLabel = (j: JavaInfo) => `Java ${j.major} · ${j.path}`;
  const detectedMatch = javas.find((j) => j.path === draft.java_path);
  const javaMode = !draft.java_path
    ? AUTO_DETECT
    : detectedMatch
      ? javaLabel(detectedMatch)
      : CUSTOM_PATH;
  const javaOptions = [AUTO_DETECT, ...javas.map(javaLabel), CUSTOM_PATH];

  const checkUpdates = async () => {
    setChecking(true);
    setCheckFailed(false);
    try {
      setUpdate(await api.checkForUpdates());
    } catch {
      setCheckFailed(true);
    } finally {
      setChecking(false);
    }
  };

  const setEnvVar = (index: number, next: EnvVar) =>
    setDraft({
      ...draft,
      env_vars: draft.env_vars.map((v, i) => (i === index ? next : v)),
    });

  const removeEnvVar = (index: number) =>
    setDraft({ ...draft, env_vars: draft.env_vars.filter((_, i) => i !== index) });

  const addEnvVar = () =>
    setDraft({ ...draft, env_vars: [...draft.env_vars, { key: "", value: "" }] });

  const refreshUsage = async () => {
    setRefreshingUsage(true);
    try {
      const usage: SystemUsage = await api.getSystemUsage();
      setStats((prev) => (prev ? { ...prev, ...usage } : prev));
    } catch (e) {
      log.warn("settings", `could not refresh system usage: ${String(e)}`);
    } finally {
      setRefreshingUsage(false);
    }
  };

  const installedMb = stats?.total_memory_mb ?? 0;
  const availableMb = stats?.available_memory_mb ?? 0;
  const memoryHint =
    installedMb > 0 && draft.max_memory_mb > installedMb
      ? `more memory than this system has (${formatGb(installedMb)})`
      : availableMb > 0 && draft.max_memory_mb > availableMb
        ? `more than is free right now (${formatGb(availableMb)})`
        : "JVM heap ceiling";

  const parseNum = (value: string, fallback: number) => {
    const n = Number(value);
    return Number.isFinite(n) && n > 0 ? Math.round(n) : fallback;
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex items-center justify-between gap-4 border-b border-border-soft px-8 py-3.5">
        <h1 className="font-display text-base font-semibold tracking-tight text-content">
          Settings
        </h1>
        <span
          className={cn(
            "inline-flex items-center gap-1.5 text-xs font-medium text-ok transition-opacity duration-300",
            saved ? "opacity-100" : "opacity-0",
          )}
        >
          <Check className="size-3.5" />
          Saved
        </span>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto px-8 py-6">
        <div
          role="tablist"
          aria-label="Settings sections"
          className="mb-6 flex flex-wrap items-center gap-6 border-b border-border-soft"
        >
          {TABS.map((entry) => (
            <button
              key={entry.id}
              role="tab"
              aria-selected={tab === entry.id}
              onClick={() => setTab(entry.id)}
              className={cn(
                "relative -mb-px pb-3 pt-1 text-sm font-medium transition-colors",
                tab === entry.id
                  ? "text-content"
                  : "text-content-faint hover:text-content-muted",
              )}
            >
              {entry.label}
              {tab === entry.id && (
                <motion.span
                  layoutId="settings-tab-underline"
                  transition={{ type: "spring", stiffness: 500, damping: 40 }}
                  className="absolute inset-x-0 -bottom-px h-0.5 rounded-full bg-[var(--accent)]"
                />
              )}
            </button>
          ))}
        </div>

        {tab === "general" && (
          <div>
        <section className="relative mb-6 overflow-hidden rounded-2xl border border-border-soft bg-surface-2/60 p-7">
          <div
            aria-hidden
            className="pointer-events-none absolute inset-0 [background:radial-gradient(90%_170%_at_0%_0%,var(--accent-glow),transparent_65%)]"
          />
          <div className="relative flex flex-wrap items-center justify-between gap-x-8 gap-y-6">
            <div className="flex min-w-0 items-center gap-6">
              <div className="size-20 shrink-0 overflow-hidden rounded-2xl shadow-lg shadow-[var(--accent-glow)]">
                <img
                  src="/logo.png"
                  alt=""
                  className="size-full object-contain"
                  draggable={false}
                />
              </div>
              <div className="min-w-0">
                <h2 className="font-display text-3xl font-semibold tracking-tight text-content">
                  Basalt
                </h2>
                <p className="mt-1 text-sm text-content-muted">
                  A fast, native Minecraft launcher.
                </p>
                <div className="mt-3.5 flex flex-wrap items-center gap-2">
                  <span className={chipCls}>
                    <Tag className="size-3.5" />
                    Version {appInfo?.version ?? "\u2026"}
                  </span>
                  {appInfo && (
                    <span className={chipCls}>
                      {appInfo.build_channel === "release"
                        ? "Release build"
                        : "Development build"}
                    </span>
                  )}
                </div>
              </div>
            </div>

            <div className="flex shrink-0 flex-col items-start gap-3">
              {update?.update_available && update.latest ? (
                <button
                  onClick={() => update.notes_url && openUrl(update.notes_url)}
                  className="inline-flex items-center gap-2 rounded-lg px-3.5 py-2 text-xs font-semibold text-black shadow-lg shadow-[var(--accent-glow)] transition-all active:scale-[0.98] [background:linear-gradient(to_bottom,var(--accent),var(--accent-deep))] hover:[background:linear-gradient(to_bottom,var(--accent-bright),var(--accent))]"
                >
                  <ArrowUpCircle className="size-4" />
                  {update.latest} is available
                </button>
              ) : (
                <div className="inline-flex items-center gap-1.5 text-xs text-content-faint">
                  {checking ? (
                    <>
                      <RefreshCw className="size-3.5 animate-spin" />
                      Checking for updates
                    </>
                  ) : checkFailed ? (
                    <span className="text-warn">Could not reach GitHub</span>
                  ) : update ? (
                    <>
                      <CircleCheck className="size-3.5 text-ok" />
                      {update.latest
                        ? "You are on the latest version"
                        : "No releases published yet"}
                    </>
                  ) : (
                    "Check whether a newer build is out"
                  )}
                </div>
              )}

              <div className="flex flex-wrap items-center gap-2">
                <button
                  onClick={() => void checkUpdates()}
                  disabled={checking}
                  className={cn(actionCls, checking && "cursor-not-allowed opacity-50")}
                >
                  <RefreshCw className={cn("size-3.5", checking && "animate-spin")} />
                  Check for updates
                </button>
                <button
                  onClick={() => links && openUrl(links.repository)}
                  className={actionCls}
                >
                  <GithubMark className="size-3.5" />
                  GitHub
                </button>
                <button
                  onClick={() => links && openUrl(links.issues)}
                  className={actionCls}
                >
                  <Bug className="size-3.5" />
                  Report an issue
                </button>
              </div>
            </div>
          </div>
        </section>

          <div className="gap-6 [column-fill:balance] lg:columns-2">
          <Section
            title="Migration"
            description="Bring instances over from another launcher."
          >
            <Row
              label="Import instances"
              hint="Copies from ATLauncher, leaving it untouched"
              stacked
            >
              <button onClick={() => setMigrateOpen(true)} className={actionCls}>
                <HardDriveDownload className="size-3.5" />
                Import
              </button>
            </Row>
          </Section>
          <Section title="Storage">
            <Row label="Data directory" hint={appInfo?.data_dir ?? "resolving"} stacked>
              <button
                onClick={() => appInfo && openPath(appInfo.data_dir)}
                className={actionCls}
              >
                <FolderOpen className="size-3.5" />
                Open folder
              </button>
            </Row>
          </Section>
          </div>

          <SystemCard
            stats={stats}
            onRefresh={() => void refreshUsage()}
            refreshing={refreshingUsage}
          />
          </div>
        )}

        {tab === "java" && (
          <div className="flex flex-wrap items-start gap-6">
          <div className="min-w-[24rem] flex-1">
          <Section
            title="Java"
            description="Basalt picks a matching runtime per version automatically. Pin one here to override everywhere."
          >
            <Row
              label="Runtime"
              hint={
                javas.length > 0
                  ? `${javas.length} detected on this system`
                  : "no runtimes detected"
              }
              stacked
            >
              <div className="w-full">
                <Select
                  value={javaMode}
                  options={javaOptions}
                  onChange={(choice) => {
                    if (choice === AUTO_DETECT) return set({ java_path: null });
                    if (choice === CUSTOM_PATH) return set({ java_path: draft.java_path ?? "" });
                    const picked = javas.find((j) => javaLabel(j) === choice);
                    if (picked) set({ java_path: picked.path });
                  }}
                />
              </div>
            </Row>
            {javaMode === CUSTOM_PATH && (
              <Row label="Custom path" hint="path to a java executable" stacked>
                <input
                  value={draft.java_path ?? ""}
                  onChange={(e) => set({ java_path: e.target.value || null })}
                  placeholder="/path/to/bin/java"
                  className={cn(inputCls, "w-full font-mono text-xs")}
                />
              </Row>
            )}
            <Row
              label="Java parameters"
              hint="the full JVM command line. Placeholders are filled in at launch."
              stacked
              action={
                appInfo && draft.jvm_args !== appInfo.default_jvm_args ? (
                  <button
                    onClick={() => set({ jvm_args: appInfo.default_jvm_args })}
                    className="text-[11px] font-medium text-content-faint transition-colors hover:text-content"
                  >
                    Reset to default
                  </button>
                ) : undefined
              }
            >
              <div className="w-full">
                <textarea
                  value={draft.jvm_args}
                  onChange={(e) => set({ jvm_args: e.target.value })}
                  spellCheck={false}
                  rows={4}
                  placeholder={appInfo?.default_jvm_args ?? ""}
                  className={cn(inputCls, "w-full resize-y font-mono text-xs leading-relaxed")}
                />
                {(appInfo?.jvm_placeholders ?? []).length > 0 && (
                  <div className="mt-3 flex flex-wrap items-center gap-1.5">
                    <span className="mr-1 text-[11px] text-content-faint">Insert</span>
                    {(appInfo?.jvm_placeholders ?? []).map((name) => (
                      <button
                        key={name}
                        title={`Insert {{${name}}} at the end`}
                        onClick={() =>
                          set({ jvm_args: `${draft.jvm_args} {{${name}}}`.trim() })
                        }
                        className="rounded-md border border-border-soft bg-surface-2 px-2 py-1 font-mono text-[10px] text-content-muted transition-colors hover:border-border hover:bg-surface-3 hover:text-content"
                      >
                        {name}
                      </button>
                    ))}
                  </div>
                )}
              </div>
            </Row>
            <Row
              label="Ignore Java checks on launch"
              hint="start anyway when the runtime is older than the version asks for"
            >
              <Toggle
                label="Ignore Java checks on launch"
                checked={draft.ignore_java_checks}
                onChange={(ignore_java_checks) => set({ ignore_java_checks })}
              />
            </Row>
          </Section>
          </div>

          <div className="min-w-[24rem] flex-1 lg:sticky lg:top-2">
            <Section
              title="Resulting command"
              description={
                preview?.pinned
                  ? "What Basalt will run. The pinned runtime above is used as is."
                  : "What Basalt will run. With auto-detect, the runtime is chosen per version."
              }
            >
              <div className="overflow-x-auto p-4">
                {preview ? (
                  <code className="block font-mono text-[11px] leading-relaxed">
                    <span className="block whitespace-pre-wrap text-content">
                      {preview.java} \
                    </span>
                    {preview.jvm.map((arg, i) => (
                      <span key={`jvm-${i}`} className="block whitespace-pre pl-4 text-content">
                        {arg} \
                      </span>
                    ))}
                    <span className="block whitespace-pre-wrap py-1 pl-4 text-content-faint/70">
                      classpath, natives path and main class are added here by Basalt
                    </span>
                    {preview.game.map((arg, i) => (
                      <span key={`game-${i}`} className="block whitespace-pre pl-4 text-content">
                        {arg}
                        {i < preview.game.length - 1 ? " \\" : ""}
                      </span>
                    ))}
                  </code>
                ) : (
                  <span className="font-mono text-[11px] text-content-faint">
                    building preview
                  </span>
                )}
              </div>
            </Section>
          </div>
          </div>
        )}

        {tab === "game" && (
          <div className="gap-6 [column-fill:balance] lg:columns-2">
          <Section
            title="Memory"
            description="Applied to every launch unless an instance overrides it."
          >
            <div className="px-5 py-5">
              <div className="mb-2 flex items-end justify-between gap-4">
                <div>
                  <div className="text-[11px] font-medium text-content-muted">Minimum</div>
                  <div className="mt-1 flex items-center gap-1.5">
                    <input
                      type="number"
                      value={draft.min_memory_mb}
                      onChange={(e) => set({ min_memory_mb: parseNum(e.target.value, 512) })}
                      className={cn(inputCls, numberCls)}
                    />
                    <span className="text-xs text-content-faint">MB</span>
                  </div>
                </div>
                <div className="text-right">
                  <div className="text-[11px] font-medium text-content-muted">Maximum</div>
                  <div className="mt-1 flex items-center gap-1.5">
                    <input
                      type="number"
                      value={draft.max_memory_mb}
                      onChange={(e) => set({ max_memory_mb: parseNum(e.target.value, 2048) })}
                      className={cn(inputCls, numberCls)}
                    />
                    <span className="text-xs text-content-faint">MB</span>
                  </div>
                </div>
              </div>

              <div className="px-1 pt-3">
                <MemoryRange
                  min={draft.min_memory_mb}
                  max={draft.max_memory_mb}
                  ceiling={Math.max(4096, stats?.total_memory_mb ?? 16384)}
                  available={stats?.available_memory_mb}
                  onChange={(low, high) =>
                    set({ min_memory_mb: low, max_memory_mb: high })
                  }
                />
              </div>

              <div className="mt-4 border-t border-border-soft pt-3 text-[11px] text-content-faint">
                {memoryHint}
              </div>
            </div>
          </Section>
          <Section
            title="Game window"
            description="How Minecraft opens. Instances inherit these unless the pack overrides them."
          >
            <Row label="Window size" hint="width and height in pixels">
              <input
                type="number"
                value={draft.window_width}
                onChange={(e) => set({ window_width: parseNum(e.target.value, 854) })}
                disabled={draft.fullscreen}
                className={cn(inputCls, numberCls, "disabled:opacity-40")}
              />
              <span className="text-xs text-content-faint">x</span>
              <input
                type="number"
                value={draft.window_height}
                onChange={(e) => set({ window_height: parseNum(e.target.value, 480) })}
                disabled={draft.fullscreen}
                className={cn(inputCls, numberCls, "disabled:opacity-40")}
              />
            </Row>
            <Row label="Start fullscreen" hint="ignores the window size above">
              <Toggle
                label="Start fullscreen"
                checked={draft.fullscreen}
                onChange={(fullscreen) => set({ fullscreen })}
              />
            </Row>
            <Row
              label="Extra game arguments"
              hint="passed to Minecraft after the launcher's own arguments"
              stacked
            >
              <input
                value={draft.game_args}
                onChange={(e) => set({ game_args: e.target.value })}
                spellCheck={false}
                placeholder="--demo"
                className={cn(inputCls, "w-full font-mono text-xs")}
              />
            </Row>
          </Section>
          <Section
            title="Environment variables"
            description="Set on the game process only. Useful for driver and GPU switches."
          >
            <div className="flex flex-col gap-2 px-5 py-4">
              {draft.env_vars.length === 0 && (
                <p className="text-xs text-content-faint">Nothing set.</p>
              )}
              {draft.env_vars.map((entry, index) => (
                <div key={index} className="flex items-center gap-2">
                  <input
                    value={entry.key}
                    onChange={(e) => setEnvVar(index, { ...entry, key: e.target.value })}
                    placeholder="MESA_GL_VERSION_OVERRIDE"
                    spellCheck={false}
                    className={cn(inputCls, "min-w-0 flex-1 font-mono text-xs")}
                  />
                  <span className="text-xs text-content-faint">=</span>
                  <input
                    value={entry.value}
                    onChange={(e) => setEnvVar(index, { ...entry, value: e.target.value })}
                    placeholder="4.5"
                    spellCheck={false}
                    className={cn(inputCls, "min-w-0 flex-1 font-mono text-xs")}
                  />
                  <button
                    onClick={() => removeEnvVar(index)}
                    title={`Remove ${entry.key || "variable"}`}
                    className="grid size-8 shrink-0 place-items-center rounded-lg text-content-faint transition-colors hover:bg-danger/15 hover:text-danger"
                  >
                    <Trash2 className="size-4" />
                  </button>
                </div>
              ))}
              <button
                onClick={addEnvVar}
                className={cn(actionCls, "mt-1 self-start")}
              >
                <Plus className="size-3.5" />
                Add variable
              </button>
            </div>
          </Section>
          </div>
        )}

        {tab === "integrations" && (
          <div>
          <Section
            title="Integrations"
            description="Modrinth works out of the box. CurseForge requires a personal key because their API is keyed per application."
          >
            <Row
              label="CurseForge API key"
              hint={draft.curseforge_api_key ? "key set" : "not set, CurseForge search disabled"}
              stacked
            >
              <input
                type="password"
                value={draft.curseforge_api_key ?? ""}
                onChange={(e) => set({ curseforge_api_key: e.target.value || null })}
                placeholder="paste your key"
                className={cn(inputCls, "min-w-0 flex-1")}
              />
              <button
                onClick={() => openUrl("https://console.curseforge.com/")}
                title="Get a key"
                className="grid size-9 shrink-0 place-items-center rounded-lg border border-border bg-surface-2 text-content-faint transition-colors hover:bg-surface-3 hover:text-content"
              >
                <KeyRound className="size-4" />
              </button>
            </Row>
          </Section>
          </div>
        )}

        {tab === "resources" && (
          <div>
          <Section
            title="Downloads"
            description="How hard Basalt pulls while installing."
          >
            <Row label="Concurrent downloads" hint="parallel files during installs">
              <input
                type="number"
                value={draft.concurrent_downloads}
                onChange={(e) =>
                  set({ concurrent_downloads: parseNum(e.target.value, 16) })
                }
                className={cn(inputCls, numberCls)}
              />
            </Row>
          </Section>
          <Section
            title="Diagnostics"
            description="Logs are written to disk and kept for seven days. Raise the level before reproducing a problem."
          >
            <Row
              label="Capture level"
              hint={
                logConfig?.env_override
                  ? `overridden by BASALT_LOG=${logConfig.env_override}`
                  : "how much detail reaches the log file"
              }
            >
              <div className="w-32">
                <Select
                  value={logConfig?.level ?? draft.log_level}
                  options={logConfig?.levels ?? ["error", "warn", "info", "debug", "trace"]}
                  onChange={(level) => void setLogLevel(level as LogLevel)}
                />
              </div>
            </Row>
            <Row label="Log file" hint={logConfig?.file ?? "resolving"} stacked>
              <button onClick={() => setView("logs")} className={actionCls}>
                <ScrollText className="size-3.5" />
                View logs
              </button>
              <button
                onClick={() => logConfig && openPath(logConfig.directory)}
                className={actionCls}
              >
                <FolderOpen className="size-3.5" />
                Open folder
              </button>
            </Row>
          </Section>
          </div>
        )}

      <MigrateModal open={migrateOpen} onClose={() => setMigrateOpen(false)} />
      </div>
    </div>
  );
}
