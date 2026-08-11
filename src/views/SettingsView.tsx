import { useEffect, useRef, useState } from "react";
import { motion } from "motion/react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { toast } from "sonner";
import {
  ArrowUpCircle,
  Bug,
  Check,
  CircleCheck,
  FolderOpen,
  HardDriveDownload,
  KeyRound,
  Loader2,
  Plus,
  Radio,
  RefreshCw,
  RotateCcw,
  ScrollText,
  Sparkles,
  Tag,
  Trash2,
  TriangleAlert,
} from "lucide-react";

import { ConfirmDialog } from "../components/ConfirmDialog";
import { MemoryRange } from "../components/MemoryRange";
import { MigrateModal } from "../components/MigrateModal";
import { Select } from "../components/Select";
import { SettingGroup, SettingRow as Row, Toggle } from "../components/ui";
import { DiscordPreview } from "../components/DiscordPreview";
import { ACCENT_PRESETS, applyTheme, DEFAULTS, isHex, themeVars } from "../lib/accent";
import { api } from "../lib/api";
import { cn } from "../lib/cn";
import { openFolder } from "../lib/reveal";
import type {
  AboutLinks,
  AccentMode,
  AppInfo,
  JavaInfo,
  EnvVar,
  LaunchPreview,
  LauncherSettings,
  LogLevel,
  NetworkProbe,
  ProxyMode,
  SystemStats,
  UpdateInfo,
} from "../lib/types";
import { StoragePanel } from "../components/storage/StoragePanel";
import { useStore } from "../store";
import { formatMegabytes } from "../lib/format";

const TABS = [
  { id: "general", label: "General" },
  { id: "java", label: "Java" },
  { id: "game", label: "Game" },
  { id: "integrations", label: "Integrations" },
  { id: "network", label: "Network" },
  { id: "appearance", label: "Look and feel" },
  { id: "resources", label: "Resources" },
  { id: "storage", label: "Storage" },
];

const DISCORD_LINES = [
  { key: "discord_rpc_show_version", label: "Version and loader" },
  { key: "discord_rpc_show_streak", label: "Streak" },
  { key: "discord_rpc_show_logo", label: "Pack logo" },
] as const;

const ACCENT_MODES: Array<{ id: AccentMode; label: string; hint: string }> = [
  {
    id: "banner",
    label: "From the banner",
    hint: "Picks up the dominant colour of the instance you have selected",
  },
  { id: "custom", label: "Custom", hint: "One colour of your choosing, everywhere" },
  { id: "default", label: "Basalt orange", hint: "The colour Basalt ships with" },
];

function ColorField({
  label,
  hint,
  value,
  fallback,
  onChange,
}: {
  label: string;
  hint: string;
  value: string;
  fallback: string;
  onChange: (next: string) => void;
}) {
  const valid = isHex(value);
  return (
    <Row label={label} hint={hint} stacked>
      <label
        className="relative size-9 shrink-0 cursor-pointer overflow-hidden rounded-lg border border-border"
        style={{ background: valid ? value : fallback }}
        title="Pick a colour"
      >
        <input
          type="color"
          value={valid ? value : fallback}
          onChange={(event) => onChange(event.target.value)}
          className="absolute inset-0 cursor-pointer opacity-0"
        />
      </label>
      <input
        value={value}
        onChange={(event) => onChange(event.target.value)}
        spellCheck={false}
        placeholder={fallback}
        className={cn(
          inputCls,
          "w-28 font-mono text-xs uppercase",
          !valid && "border-danger/50 text-danger",
        )}
      />
      {value.toLowerCase() !== fallback.toLowerCase() && (
        <button
          onClick={() => onChange(fallback)}
          title="Back to the default"
          className="grid size-9 shrink-0 place-items-center rounded-lg border border-border bg-surface-2 text-content-faint transition-colors hover:bg-surface-3 hover:text-content"
        >
          <RotateCcw className="size-3.5" />
        </button>
      )}
    </Row>
  );
}

function Section(props: React.ComponentProps<typeof SettingGroup>) {
  return <SettingGroup {...props} className={cn("pb-6", props.className)} />;
}

const PROXY_LABELS: Record<ProxyMode, string> = {
  system: "Use system settings",
  none: "Direct, no proxy",
  http: "HTTP proxy",
  socks5: "SOCKS5 proxy",
};

const AUTO_DETECT = "Auto-detect";
const CUSTOM_PATH = "Custom path";

const inputCls =
  "rounded-lg border border-border bg-void px-3 py-2 text-sm text-content outline-none transition-colors placeholder:text-content-faint focus:border-(--accent)";

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

function DiscordMark({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden className={className}>
      <path d="M20.317 4.37a19.79 19.79 0 0 0-4.885-1.515.074.074 0 0 0-.079.037c-.21.375-.444.865-.608 1.25a18.27 18.27 0 0 0-5.487 0 12.64 12.64 0 0 0-.617-1.25.077.077 0 0 0-.079-.037A19.74 19.74 0 0 0 3.677 4.37a.07.07 0 0 0-.032.027C.533 9.046-.32 13.58.099 18.058a.082.082 0 0 0 .031.056 19.9 19.9 0 0 0 5.993 3.03.078.078 0 0 0 .084-.028c.462-.63.874-1.295 1.226-1.994a.076.076 0 0 0-.041-.106 13.1 13.1 0 0 1-1.872-.892.077.077 0 0 1-.008-.128c.126-.094.252-.192.372-.291a.074.074 0 0 1 .078-.011c3.928 1.793 8.18 1.793 12.061 0a.074.074 0 0 1 .079.01c.12.099.246.198.373.292a.077.077 0 0 1-.007.128c-.598.349-1.22.645-1.873.891a.077.077 0 0 0-.04.107c.36.698.772 1.362 1.225 1.993a.076.076 0 0 0 .084.029 19.84 19.84 0 0 0 6.002-3.03.077.077 0 0 0 .032-.055c.5-5.177-.838-9.674-3.549-13.66a.06.06 0 0 0-.031-.029ZM8.02 15.331c-1.183 0-2.157-1.086-2.157-2.419 0-1.333.955-2.419 2.157-2.419 1.211 0 2.176 1.095 2.157 2.419 0 1.333-.956 2.419-2.157 2.419Zm7.975 0c-1.183 0-2.157-1.086-2.157-2.419 0-1.333.955-2.419 2.157-2.419 1.211 0 2.176 1.095 2.157 2.419 0 1.333-.946 2.419-2.157 2.419Z" />
    </svg>
  );
}


export function SettingsView() {
  const [migrateOpen, setMigrateOpen] = useState(false);
  const [probe, setProbe] = useState<NetworkProbe | null>(null);
  const [resetting, setResetting] = useState(false);
  const [deepReset, setDeepReset] = useState(false);
  const [probing, setProbing] = useState(false);
  const settings = useStore((s) => s.settings);
  const logConfig = useStore((s) => s.logConfig);
  const setLogLevel = useStore((s) => s.setLogLevel);
  const setView = useStore((s) => s.setView);
  const appUpdateStatus = useStore((s) => s.appUpdateStatus);
  const downloadAppUpdate = useStore((s) => s.downloadAppUpdate);
  const installAppUpdate = useStore((s) => s.installAppUpdate);
  const bannerAccent = useStore((s) =>
    s.selectedInstanceId ? (s.media[s.selectedInstanceId]?.accent ?? null) : null,
  );
  const [draft, setDraft] = useState<LauncherSettings | null>(settings);
  const [javas, setJavas] = useState<JavaInfo[]>([]);
  const [javaMajor, setJavaMajor] = useState(21);
  const [installingJava, setInstallingJava] = useState(false);
  const [javaInstallError, setJavaInstallError] = useState<string | null>(null);
  const [installedJava, setInstalledJava] = useState<JavaInfo | null>(null);
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const [saved, setSaved] = useState(false);
  const [stats, setStats] = useState<SystemStats | null>(null);
  const [preview, setPreview] = useState<LaunchPreview | null>(null);
  const [tab, setTab] = useState(TABS[0].id);
  const [links, setLinks] = useState<AboutLinks | null>(null);
  const [update, setUpdate] = useState<UpdateInfo | null>(null);
  const [checking, setChecking] = useState(false);
  const [checkFailed, setCheckFailed] = useState(false);
  const [reconnecting, setReconnecting] = useState(false);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const firstRun = useRef(true);
  const discordReady =
    (draft?.discord_app_id ?? "").trim().length > 0 || appInfo?.bundled_discord_app_id === true;

  useEffect(() => setDraft(settings), [settings]);

  useEffect(() => {
    api.listJavas().then((list) => setJavas(list ?? [])).catch(() => {});
    api.getAppInfo().then(setAppInfo).catch(() => {});
    api.getAboutLinks().then(setLinks).catch(() => {});
    api.getSystemStats().then(setStats).catch(() => {});
  }, []);

  useEffect(() => {
    const refreshJavas = () =>
      api
        .listJavas()
        .then((list) => setJavas(list ?? []))
        .catch(() => {});
    window.addEventListener("focus", refreshJavas);
    return () => window.removeEventListener("focus", refreshJavas);
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
      } catch (error) {
        toast.error("Could not save settings", {
          id: "settings-save-error",
          description: String(error),
        });
        return;
      }
    }, 500);
    return () => clearTimeout(debounceRef.current);
  }, [draft]);

  useEffect(() => {
    if (draft) applyTheme(themeVars(draft, bannerAccent));
  }, [draft, bannerAccent]);

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

  const installJava = async () => {
    if (installingJava) return;
    setInstallingJava(true);
    setJavaInstallError(null);
    setInstalledJava(null);
    try {
      const installed = await api.installJavaRuntime(javaMajor);
      setJavas(await api.listJavas());
      setInstalledJava(installed);
    } catch (error) {
      setJavaInstallError(String(error));
    } finally {
      setInstallingJava(false);
    }
  };

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

  const applyUpdate = async () => {
    const info = appUpdateStatus?.info ?? update;
    if (!info?.update_available || !info.latest) return;
    if (appUpdateStatus?.phase === "downloading") return;
    if (appUpdateStatus?.phase === "ready") {
      try {
        await installAppUpdate();
      } catch (error) {
        toast.error("Basalt could not restart for the update", {
          description: String(error),
        });
      }
      return;
    }
    if (info.install_source.policy !== "self_managed") {
      if (info.notes_url) await openUrl(info.notes_url);
      return;
    }
    try {
      await downloadAppUpdate();
    } catch (error) {
      toast.error("Update download failed", { description: String(error) });
    }
  };

  const visibleUpdate = appUpdateStatus?.info ?? update;
  const updateDownloading = appUpdateStatus?.phase === "downloading";
  const updateReady = appUpdateStatus?.phase === "ready";

  const setEnvVar = (index: number, next: EnvVar) =>
    setDraft({
      ...draft,
      env_vars: draft.env_vars.map((v, i) => (i === index ? next : v)),
    });

  const removeEnvVar = (index: number) =>
    setDraft({ ...draft, env_vars: draft.env_vars.filter((_, i) => i !== index) });

  const addEnvVar = () =>
    setDraft({ ...draft, env_vars: [...draft.env_vars, { key: "", value: "" }] });

  const installedMb = stats?.total_memory_mb ?? 0;
  const availableMb = stats?.available_memory_mb ?? 0;
  const memoryHint =
    installedMb > 0 && draft.max_memory_mb > installedMb
      ? `more memory than this system has (${formatMegabytes(installedMb)})`
      : availableMb > 0 && draft.max_memory_mb > availableMb
        ? `more than is free right now (${formatMegabytes(availableMb)})`
        : "JVM heap ceiling";

  const parseNum = (value: string, fallback: number) => {
    const n = Number(value);
    return Number.isFinite(n) && n > 0 ? Math.round(n) : fallback;
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex items-center justify-between gap-4 border-b border-border-soft px-8 py-3.5">
        <h1 className="font-display text-[1rem] font-semibold tracking-tight text-content">
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
                  className="absolute inset-x-0 -bottom-px h-0.5 rounded-full bg-(--accent)"
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
              <div className="size-20 shrink-0 overflow-hidden rounded-2xl shadow-lg shadow-(color:--accent-glow)">
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
              {visibleUpdate?.update_available && visibleUpdate.latest ? (
                <div className="flex max-w-80 flex-col items-start gap-1.5">
                  <button
                    onClick={() => void applyUpdate()}
                    disabled={updateDownloading}
                    className="inline-flex items-center gap-2 rounded-lg px-3.5 py-2 text-xs font-semibold text-black shadow-lg shadow-(color:--accent-glow) transition-all active:scale-[0.98] disabled:cursor-wait disabled:opacity-70 [background:linear-gradient(to_bottom,var(--accent),var(--accent-deep))] hover:[background:linear-gradient(to_bottom,var(--accent-bright),var(--accent))]"
                  >
                    {updateDownloading ? (
                      <Loader2 className="size-4 animate-spin" />
                    ) : (
                      <ArrowUpCircle className="size-4" />
                    )}
                    {updateDownloading
                      ? "Downloading in Activity Center"
                      : updateReady
                        ? "Restart and update"
                        : visibleUpdate.install_source.policy === "self_managed"
                          ? `Download ${visibleUpdate.latest}`
                          : `View ${visibleUpdate.latest}`}
                  </button>
                  <span className="text-[11px] leading-relaxed text-content-faint">
                    {updateReady
                      ? "The signed update is verified and ready to install."
                      : visibleUpdate.install_source.update_hint}
                  </span>
                </div>
              ) : (
                <div className="inline-flex items-center gap-1.5 text-xs text-content-faint">
                  {checking ? (
                    <>
                      <RefreshCw className="size-3.5 animate-spin" />
                      Checking for updates
                    </>
                  ) : checkFailed ? (
                    <span className="text-warn">Could not reach GitHub</span>
                  ) : visibleUpdate ? (
                    <>
                      <CircleCheck className="size-3.5 text-ok" />
                      {visibleUpdate.latest
                        ? "You are on the latest version"
                        : "No releases published yet"}
                    </>
                  ) : (
                    "Check whether a newer build is out"
                  )}
                </div>
              )}

              <div className="flex items-center gap-2">
                <button
                  onClick={() => void checkUpdates()}
                  disabled={checking}
                  className={cn(
                    "inline-flex h-9 items-center gap-2 rounded-lg border border-border bg-surface-2 px-3.5 text-xs font-medium text-content transition-colors hover:bg-surface-3",
                    checking && "cursor-not-allowed opacity-50",
                  )}
                >
                  <RefreshCw className={cn("size-3.5", checking && "animate-spin")} />
                  Check for updates
                </button>

                <div className="flex h-9 items-center overflow-hidden rounded-lg border border-border bg-surface-2">
                  <button
                    onClick={() => links && openUrl(links.repository)}
                    aria-label="Open the repository"
                    title="Open the repository"
                    className="grid h-full w-10 place-items-center text-content-faint transition-colors hover:bg-surface-3 hover:text-content"
                  >
                    <GithubMark className="size-4" />
                  </button>
                  <span className="h-5 w-px bg-border" />
                  <button
                    onClick={() => links && openUrl(links.issues)}
                    aria-label="Report an issue"
                    title="Report an issue"
                    className="grid h-full w-10 place-items-center text-content-faint transition-colors hover:bg-surface-3 hover:text-content"
                  >
                    <Bug className="size-4" />
                  </button>
                  <span className="h-5 w-px bg-border" />
                  <button
                    onClick={() => links && openUrl(links.discord)}
                    aria-label="Join the Discord server"
                    title="Join the Discord server"
                    className="grid h-full w-10 place-items-center text-content-faint transition-colors hover:bg-surface-3 hover:text-content"
                  >
                    <DiscordMark className="size-4" />
                  </button>
                </div>
              </div>
            </div>
          </div>
        </section>

          <div className="grid items-start gap-x-6 lg:grid-cols-2">
          <div>
          <Section
            title="Updates"
            description="Choose how Basalt looks for new launcher versions."
          >
            <Row
              label="Automatically check for updates"
              hint="Checks periodically in the background. You can still check manually above."
            >
              <Toggle
                label="Automatically check for updates"
                checked={draft.auto_update_checks}
                onChange={(auto_update_checks) => set({ auto_update_checks })}
              />
            </Row>
          </Section>
          </div>

          <div>
          <Section
            title="Migration"
            description="Bring instances over from another launcher."
          >
            <Row
              label="Import instances"
              hint="Copies from ATLauncher, Prism or the Modrinth App, leaving them untouched"
              stacked
            >
              <button onClick={() => setMigrateOpen(true)} className={actionCls}>
                <HardDriveDownload className="size-3.5" />
                Import
              </button>
            </Row>
            {appInfo?.build_channel === "dev" && (
              <Row
                label="Run setup again"
                hint="Walks through the first launch steps from the start"
                stacked
              >
                <button onClick={() => set({ onboarded: false })} className={actionCls}>
                  <Sparkles className="size-3.5" />
                  Start setup
                </button>
              </Row>
            )}
          </Section>
          </div>
          </div>

          <p className="mt-10 border-t border-border-soft pt-5 text-[11px] leading-relaxed text-content-faint">
            Not an official Minecraft product. Not approved by or associated with Mojang
            or Microsoft.
          </p>
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
              label="Managed runtimes"
              hint="Download Eclipse Temurin into Basalt without changing system Java"
              stacked
            >
              <div className="w-full space-y-3">
                <div className="flex flex-wrap items-center gap-2">
                  {[8, 16, 17, 21, 25].map((major) => (
                    <button
                      key={major}
                      onClick={() => {
                        setJavaMajor(major);
                        setJavaInstallError(null);
                        setInstalledJava(null);
                      }}
                      disabled={installingJava}
                      className={cn(
                        "rounded-lg border px-3 py-1.5 text-xs font-medium transition-colors disabled:opacity-60",
                        javaMajor === major
                          ? "border-(--accent)/50 bg-(--accent-glow) text-(--accent-bright)"
                          : "border-border bg-surface-2 text-content-muted hover:text-content",
                      )}
                    >
                      Java {major}
                    </button>
                  ))}
                  <button
                    onClick={() => void installJava()}
                    disabled={installingJava}
                    className={cn(actionCls, "ml-auto")}
                  >
                    {installingJava ? (
                      <Loader2 className="size-3.5 animate-spin" />
                    ) : (
                      <HardDriveDownload className="size-3.5" />
                    )}
                    {installingJava ? "Downloading" : `Download Java ${javaMajor}`}
                  </button>
                </div>
                <div className="flex items-center justify-between gap-3 text-[11px]">
                  <span
                    className={cn(
                      javaInstallError
                        ? "text-danger"
                        : installedJava
                          ? "text-ok"
                          : "text-content-faint",
                    )}
                  >
                    {javaInstallError ??
                      (installedJava
                        ? `Java ${installedJava.major} is ready and available above.`
                        : "Downloads are verified and can resume after an interruption.")}
                  </span>
                  <button
                    onClick={() =>
                      void openUrl(`https://adoptium.net/temurin/releases/?version=${javaMajor}`)
                    }
                    disabled={installingJava}
                    className="shrink-0 font-medium text-content-muted transition-colors hover:text-content disabled:opacity-60"
                  >
                    Install manually
                  </button>
                </div>
              </div>
            </Row>
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
                  <code className="selectable block font-mono text-[11px] leading-relaxed">
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
            title="Around the launch"
            description="Commands Basalt runs for you. Placeholders like {{instance_name}} and {{game_dir}} work here."
          >
            <Row
              label="Wrapper command"
              hint="runs the game through this, for example mangohud or gamemoderun"
              stacked
            >
              <input
                value={draft.wrapper_command}
                onChange={(e) => set({ wrapper_command: e.target.value })}
                spellCheck={false}
                placeholder="mangohud"
                className={cn(inputCls, "w-full font-mono text-xs")}
              />
            </Row>
            <Row
              label="Before launching"
              hint="the launch stops if this fails, and its output is shown"
              stacked
            >
              <input
                value={draft.pre_launch_command}
                onChange={(e) => set({ pre_launch_command: e.target.value })}
                spellCheck={false}
                placeholder="systemctl --user start my-backup.service"
                className={cn(inputCls, "w-full font-mono text-xs")}
              />
            </Row>
            <Row
              label="After the game exits"
              hint="failures are written to the log and nothing else"
              stacked
            >
              <input
                value={draft.post_exit_command}
                onChange={(e) => set({ post_exit_command: e.target.value })}
                spellCheck={false}
                placeholder="notify-send 'Minecraft closed'"
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
                onFocus={(event) => event.currentTarget.select()}
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
          <Section
            title="Discord"
            description="Shows what you are playing on your Discord profile while an instance is running, and what you are doing in the launcher (browsing modpacks, reading logs, ...) when no game is running. Nothing leaves your machine when Discord is not open."
          >
            <div className="flex flex-col gap-6 px-4 py-4 lg:flex-row lg:items-start">
              <DiscordPreview
                enabled={draft.discord_rpc && discordReady}
                showVersion={draft.discord_rpc_show_version}
                showStreak={draft.discord_rpc_show_streak}
                showLogo={draft.discord_rpc_show_logo}
              />
              <div className="min-w-0 flex-1">
                <label className="flex items-center justify-between gap-4 pb-3">
                  <span className="text-sm font-medium text-content">
                    Show what I am playing
                  </span>
                  <Toggle
                    label="Show what I am playing"
                    checked={draft.discord_rpc}
                    onChange={(discord_rpc) => set({ discord_rpc })}
                  />
                </label>

                <div
                  className={cn(
                    "space-y-2.5 border-t border-border-soft py-3 pl-4 transition-opacity",
                    !draft.discord_rpc && "opacity-50",
                  )}
                >
                  {DISCORD_LINES.map((line) => (
                    <label key={line.key} className="flex items-center justify-between gap-4">
                      <span className="text-[13px] text-content-muted">{line.label}</span>
                      <Toggle
                        label={line.label}
                        checked={draft[line.key]}
                        onChange={(value) => set({ [line.key]: value })}
                        disabled={!draft.discord_rpc}
                      />
                    </label>
                  ))}
                </div>

                <div className="border-t border-border-soft pt-3">
                  <div className="text-[13px] text-content-muted">Application id</div>
                  <div className="mt-2 flex items-center gap-2">
                    <input
                      value={draft.discord_app_id}
                      onChange={(e) => set({ discord_app_id: e.target.value })}
                      placeholder="leave empty to use the one this build ships with"
                      className={cn(inputCls, "min-w-0 flex-1")}
                    />
                    <button
                      onClick={() => openUrl("https://discord.com/developers/applications")}
                      title="Discord developer portal"
                      className="grid size-9 shrink-0 place-items-center rounded-lg border border-border bg-surface-2 text-content-faint transition-colors hover:bg-surface-3 hover:text-content"
                    >
                      <KeyRound className="size-4" />
                    </button>
                    <button
                      onClick={async () => {
                        setReconnecting(true);
                        try {
                          await api.reconnectDiscord();
                          toast.success("Connected to Discord");
                        } catch (error) {
                          toast.error("Discord refused the connection", {
                            description: String(error),
                          });
                        } finally {
                          setReconnecting(false);
                        }
                      }}
                      disabled={!draft.discord_rpc || !discordReady || reconnecting}
                      className={cn(actionCls, "disabled:cursor-not-allowed disabled:opacity-50")}
                    >
                      {reconnecting ? (
                        <Loader2 className="size-3.5 animate-spin" />
                      ) : (
                        <RefreshCw className="size-3.5" />
                      )}
                      Reconnect
                    </button>
                  </div>
                  {!discordReady && (
                    <p className="mt-2 text-[12px] text-warn">
                      Discord shows nothing until an application id is set.
                    </p>
                  )}
                </div>
              </div>
            </div>
          </Section>
          </div>
        )}

        {tab === "network" && (
          <div className="gap-6 [column-fill:balance] lg:columns-2">
          <Section
            title="Proxy"
            description="Routes every request Basalt makes. The game itself is not affected."
          >
            <Row label="Mode" hint="System follows the environment variables">
              <div className="w-44">
                <Select
                  compact
                  value={PROXY_LABELS[draft.proxy_mode] ?? PROXY_LABELS.system}
                  options={Object.values(PROXY_LABELS)}
                  onChange={(label) => {
                    const mode = (Object.keys(PROXY_LABELS) as ProxyMode[]).find(
                      (key) => PROXY_LABELS[key] === label,
                    );
                    set({ proxy_mode: mode ?? "system" });
                    setProbe(null);
                  }}
                />
              </div>
            </Row>
            {(draft.proxy_mode === "http" || draft.proxy_mode === "socks5") && (
              <>
                <Row label="Address" hint="host name or address of the proxy">
                  <input
                    value={draft.proxy_host}
                    onChange={(e) => set({ proxy_host: e.target.value })}
                    placeholder="127.0.0.1"
                    spellCheck={false}
                    className={cn(inputCls, "w-56 font-mono text-xs")}
                  />
                </Row>
                <Row label="Port" hint={draft.proxy_mode === "socks5" ? "1080 by default" : "8080 by default"}>
                  <input
                    type="number"
                    value={draft.proxy_port || ""}
                    onChange={(e) => set({ proxy_port: parseNum(e.target.value, 0) })}
                    placeholder={draft.proxy_mode === "socks5" ? "1080" : "8080"}
                    className={cn(inputCls, numberCls)}
                  />
                </Row>
                <Row label="Username" hint="leave empty if the proxy is open">
                  <input
                    value={draft.proxy_username}
                    onChange={(e) => set({ proxy_username: e.target.value })}
                    spellCheck={false}
                    className={cn(inputCls, "w-56")}
                  />
                </Row>
                <Row label="Password">
                  <input
                    type="password"
                    value={draft.proxy_password}
                    onFocus={(event) => event.currentTarget.select()}
                    onChange={(e) => set({ proxy_password: e.target.value })}
                    className={cn(inputCls, "w-56")}
                  />
                </Row>
              </>
            )}
            <Row
              label="Test the connection"
              hint={
                probe
                  ? probe.ok
                    ? `Reached Modrinth in ${probe.millis} ms${probe.via_proxy ? " through the proxy" : ""}`
                    : (probe.error ?? "The request failed")
                  : "Saves first, then asks Modrinth for a small response"
              }
              stacked
              action={
                probe && (
                  <span
                    className={cn(
                      "inline-flex items-center gap-1.5 text-xs font-medium",
                      probe.ok ? "text-ok" : "text-danger",
                    )}
                  >
                    {probe.ok ? (
                      <CircleCheck className="size-3.5" />
                    ) : (
                      <TriangleAlert className="size-3.5" />
                    )}
                    {probe.ok ? "Reachable" : "No answer"}
                  </span>
                )
              }
            >
              <button
                onClick={async () => {
                  setProbing(true);
                  setProbe(null);
                  try {
                    await api.updateSettings(draft);
                    setProbe(await api.testNetwork());
                  } catch (e) {
                    setProbe({
                      ok: false,
                      status: null,
                      millis: 0,
                      via_proxy: false,
                      error: String(e),
                    });
                  } finally {
                    setProbing(false);
                  }
                }}
                disabled={probing}
                className={cn(actionCls, "disabled:opacity-50")}
              >
                {probing ? (
                  <Loader2 className="size-3.5 animate-spin" />
                ) : (
                  <Radio className="size-3.5" />
                )}
                Test connection
              </button>
            </Row>
          </Section>

          <Section
            title="Requests"
            description="How patient Basalt is with slow or flaky servers."
          >
            <Row label="Timeout" hint="how long a single request may take">
              <input
                type="number"
                value={draft.request_timeout_secs}
                onChange={(e) => set({ request_timeout_secs: parseNum(e.target.value, 45) })}
                className={cn(inputCls, numberCls)}
              />
              <span className="text-xs text-content-faint">sec</span>
            </Row>
            <Row label="Retries" hint="attempts before a download is given up on">
              <input
                type="number"
                value={draft.max_retries}
                onChange={(e) => set({ max_retries: parseNum(e.target.value, 4) })}
                className={cn(inputCls, numberCls)}
              />
            </Row>
            <Row
              label="Accept invalid certificates"
              hint="Only for a proxy that inspects traffic with its own certificate. Leave off otherwise."
            >
              <Toggle
                label="Accept invalid certificates"
                checked={draft.allow_insecure_tls}
                onChange={(v) => set({ allow_insecure_tls: v })}
              />
            </Row>
          </Section>
          </div>
        )}

        {tab === "appearance" && (
          <div className="gap-6 [column-fill:balance] lg:columns-2">
          <Section
            title="Accent colour"
            description="The colour Basalt uses for buttons, highlights and progress."
          >
            {ACCENT_MODES.map((mode) => (
              <Row key={mode.id} label={mode.label} hint={mode.hint}>
                <button
                  onClick={() => set({ accent_mode: mode.id })}
                  aria-pressed={draft.accent_mode === mode.id}
                  className={cn(
                    "grid size-5 shrink-0 place-items-center rounded-full border transition-colors",
                    draft.accent_mode === mode.id
                      ? "border-(--accent) bg-(--accent) text-black"
                      : "border-border bg-surface-3 hover:border-content-faint",
                  )}
                >
                  {draft.accent_mode === mode.id && <Check className="size-3" strokeWidth={4} />}
                </button>
              </Row>
            ))}

            {draft.accent_mode === "custom" && (
              <>
                <Row label="Presets" hint="A starting point you can fine tune below" stacked>
                  <div className="flex flex-wrap gap-2">
                    {ACCENT_PRESETS.map((preset) => (
                      <button
                        key={preset}
                        onClick={() => set({ accent_color: preset })}
                        aria-label={preset}
                        title={preset}
                        style={{ background: preset }}
                        className={cn(
                          "grid size-7 place-items-center rounded-lg border transition-transform hover:scale-105",
                          draft.accent_color.toLowerCase() === preset
                            ? "border-content"
                            : "border-border",
                        )}
                      >
                        {draft.accent_color.toLowerCase() === preset && (
                          <Check className="size-3.5 text-black" strokeWidth={3} />
                        )}
                      </button>
                    ))}
                  </div>
                </Row>
                <ColorField
                  label="Accent"
                  hint="Used for the play button, links, progress and focus rings"
                  value={draft.accent_color}
                  fallback={DEFAULTS.accent}
                  onChange={(accent_color) => set({ accent_color })}
                />
              </>
            )}
          </Section>

          <Section
            title="Status colours"
            description="What Basalt uses to say something went well, needs attention, or failed."
          >
            <ColorField
              label="Success"
              hint="Verified downloads, a running instance, finished tasks"
              value={draft.ok_color}
              fallback={DEFAULTS.ok}
              onChange={(ok_color) => set({ ok_color })}
            />
            <ColorField
              label="Warning"
              hint="Available updates, loader changes, recovered worlds"
              value={draft.warn_color}
              fallback={DEFAULTS.warn}
              onChange={(warn_color) => set({ warn_color })}
            />
            <ColorField
              label="Danger"
              hint="Deletions, failed tasks, the reset button"
              value={draft.danger_color}
              fallback={DEFAULTS.danger}
              onChange={(danger_color) => set({ danger_color })}
            />
          </Section>

          <Section title="Behaviour" description="How Basalt acts while you use it.">
            <Row
              label="Suggest content while searching"
              hint="When an instance search finds nothing installed, look for matches on Modrinth and CurseForge"
            >
              <Toggle
                label="Suggest content while searching"
                checked={draft.show_suggestions}
                onChange={(show_suggestions) => set({ show_suggestions })}
              />
            </Row>
            <Row
              label="Update pack content too"
              hint="Modpack instances normally only offer updates for content you added yourself, since the pack replaces its own files on the next upgrade"
            >
              <Toggle
                label="Update pack content too"
                checked={draft.pack_content_updates}
                onChange={(pack_content_updates) => set({ pack_content_updates })}
              />
            </Row>
            <Row
              label="Minimize while the game runs"
              hint="Gets Basalt out of the way on launch and brings it back when the last instance closes"
            >
              <Toggle
                label="Minimize while the game runs"
                checked={draft.minimize_on_launch}
                onChange={(minimize_on_launch) => set({ minimize_on_launch })}
              />
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
                onClick={() => logConfig && openFolder(logConfig.directory)}
                className={actionCls}
              >
                <FolderOpen className="size-3.5" />
                Open folder
              </button>
            </Row>
          </Section>
          </div>
        )}

        {tab === "storage" && (
          <div>
            <StoragePanel />
            <div className="mt-8 flex flex-wrap items-center gap-x-4 gap-y-3 rounded-2xl border border-border-soft bg-surface-2/60 px-4 py-3">
              <div className="min-w-0 flex-1">
                <p className="text-[10px] font-semibold uppercase tracking-wider text-content-faint">
                  Data directory
                </p>
                <p className="selectable mt-0.5 break-all font-mono text-[11px] text-content-muted">
                  {appInfo?.data_dir ?? "resolving"}
                </p>
              </div>
              <button
                onClick={() => appInfo && openFolder(appInfo.data_dir)}
                className={actionCls}
              >
                <FolderOpen className="size-3.5" />
                Open folder
              </button>
              <span className="h-6 w-px shrink-0 bg-border-soft" />
              <button
                onClick={() => setResetting(true)}
                title="Removes every instance with its worlds, the accounts, the skins and all settings, then restarts into setup"
                className="inline-flex shrink-0 items-center gap-1.5 rounded-lg px-3 py-2 text-xs font-medium text-content-faint transition-colors hover:bg-danger/10 hover:text-danger"
              >
                <TriangleAlert className="size-3.5" />
                Reset Basalt
              </button>
            </div>
          </div>
        )}

      <MigrateModal open={migrateOpen} onClose={() => setMigrateOpen(false)} />

      <ConfirmDialog
        open={resetting}
        title="Reset Basalt?"
        description="Every instance goes, with its worlds, mods and configs. Accounts, skins and settings go with them. Basalt restarts into first time setup."
        confirmLabel="Reset and restart"
        requireText="reset"
        onConfirm={async () => {
          await api.resetLauncher(deepReset);
        }}
        onCancel={() => {
          setResetting(false);
          setDeepReset(false);
        }}
      >
        <button
          onClick={() => setDeepReset((v) => !v)}
          className="flex w-full items-center gap-2.5 rounded-lg px-1.5 py-1.5 text-left transition-colors hover:bg-surface-2"
        >
          <span
            className={cn(
              "grid size-4 shrink-0 place-items-center rounded border",
              deepReset ? "border-danger bg-danger/20 text-danger" : "border-border bg-surface-3",
            )}
          >
            {deepReset && <Check className="size-3" strokeWidth={3} />}
          </span>
          <span className="min-w-0 flex-1">
            <span className="block text-xs font-medium text-content">
              Also remove downloaded game files
            </span>
            <span className="block text-[11px] text-content-faint">
              Versions, libraries, assets and Java runtimes are fetched again on the next launch
            </span>
          </span>
        </button>
      </ConfirmDialog>
      </div>
    </div>
  );
}
