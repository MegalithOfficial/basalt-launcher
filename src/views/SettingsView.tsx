import { useEffect, useRef, useState } from "react";
import { openPath, openUrl } from "@tauri-apps/plugin-opener";
import {
  ArrowUpCircle,
  Bug,
  Check,
  CircleCheck,
  Coffee,
  Database,
  FolderOpen,
  KeyRound,
  RefreshCw,
  ScrollText,
  Tag,
} from "lucide-react";

import { Select } from "../components/Select";
import { api } from "../lib/api";
import { cn } from "../lib/cn";
import type {
  AboutLinks,
  AppInfo,
  JavaInfo,
  LauncherSettings,
  LogLevel,
  UpdateInfo,
} from "../lib/types";
import { useStore } from "../store";

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

function Section({
  title,
  description,
  children,
}: {
  title: string;
  description?: string;
  children: React.ReactNode;
}) {
  return (
    <section className="mb-6 break-inside-avoid">
      <div className="mb-2 px-1">
        <h2 className="font-display text-sm font-semibold text-content">{title}</h2>
        {description && <p className="mt-0.5 text-xs text-content-muted">{description}</p>}
      </div>
      <div className="divide-y divide-border-soft rounded-2xl border border-border-soft bg-surface-2/60">
        {children}
      </div>
    </section>
  );
}

function Row({
  label,
  hint,
  children,
  stacked,
}: {
  label: string;
  hint?: string;
  children: React.ReactNode;
  stacked?: boolean;
}) {
  if (stacked) {
    return (
      <div className="px-5 py-4">
        <div className="text-sm font-medium text-content">{label}</div>
        {hint && (
          <div className="mt-0.5 break-words text-xs text-content-faint">{hint}</div>
        )}
        <div className="mt-3 flex items-center gap-2">{children}</div>
      </div>
    );
  }
  return (
    <div className="flex items-center justify-between gap-5 px-5 py-4">
      <div className="min-w-0">
        <div className="text-sm font-medium text-content">{label}</div>
        {hint && (
          <div className="mt-0.5 break-words text-xs text-content-faint">{hint}</div>
        )}
      </div>
      <div className="flex shrink-0 items-center gap-2">{children}</div>
    </div>
  );
}

export function SettingsView() {
  const settings = useStore((s) => s.settings);
  const logConfig = useStore((s) => s.logConfig);
  const setLogLevel = useStore((s) => s.setLogLevel);
  const setView = useStore((s) => s.setView);
  const [draft, setDraft] = useState<LauncherSettings | null>(settings);
  const [javas, setJavas] = useState<JavaInfo[]>([]);
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const [saved, setSaved] = useState(false);
  const [links, setLinks] = useState<AboutLinks | null>(null);
  const [update, setUpdate] = useState<UpdateInfo | null>(null);
  const [checking, setChecking] = useState(false);
  const [checkFailed, setCheckFailed] = useState(false);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const firstRun = useRef(true);

  useEffect(() => setDraft(settings), [settings]);

  useEffect(() => {
    api.listJavas().then(setJavas).catch(() => {});
    api.getAppInfo().then(setAppInfo).catch(() => {});
    api.getAboutLinks().then(setLinks).catch(() => {});
  }, []);

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
        <section className="relative mb-8 overflow-hidden rounded-2xl border border-border-soft bg-surface-2/60 p-7">
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
                  className="size-full scale-[1.45] object-cover"
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
                  <span className={chipCls}>
                    <Coffee className="size-3.5" />
                    {javas.length} java runtime{javas.length === 1 ? "" : "s"}
                  </span>
                  <span className={chipCls}>
                    <Database className="size-3.5" />
                    basalt.db
                  </span>
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

        <div className="gap-6 [column-fill:balance] lg:columns-2 2xl:columns-3">
          <Section
            title="Game defaults"
            description="Applied to every launch unless an instance overrides them."
          >
            <Row label="Minimum memory" hint="JVM initial heap">
              <input
                type="number"
                value={draft.min_memory_mb}
                onChange={(e) => set({ min_memory_mb: parseNum(e.target.value, 512) })}
                className={cn(inputCls, numberCls)}
              />
              <span className="text-xs text-content-faint">MB</span>
            </Row>
            <Row label="Maximum memory" hint="JVM heap ceiling">
              <input
                type="number"
                value={draft.max_memory_mb}
                onChange={(e) => set({ max_memory_mb: parseNum(e.target.value, 2048) })}
                className={cn(inputCls, numberCls)}
              />
              <span className="text-xs text-content-faint">MB</span>
            </Row>
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
          </Section>

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
      </div>
    </div>
  );
}
