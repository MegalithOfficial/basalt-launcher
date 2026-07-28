import { useEffect, useRef, useState } from "react";
import { openPath, openUrl } from "@tauri-apps/plugin-opener";
import { Check, Coffee, Database, FolderOpen, KeyRound, ScrollText } from "lucide-react";

import { PageHeader } from "../components/ui";
import { Select } from "../components/Select";
import { api } from "../lib/api";
import { cn } from "../lib/cn";
import type { AppInfo, JavaInfo, LauncherSettings, LogLevel } from "../lib/types";
import { useStore } from "../store";

const AUTO_DETECT = "Auto-detect";
const CUSTOM_PATH = "Custom path";

const inputCls =
  "rounded-lg border border-border bg-base px-3 py-2 text-sm text-content outline-none transition-colors focus:border-[var(--accent)]";

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
    <section>
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
}: {
  label: string;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex items-center justify-between gap-6 px-5 py-4">
      <div className="min-w-0">
        <div className="text-sm font-medium text-content">{label}</div>
        {hint && <div className="mt-0.5 text-xs text-content-faint">{hint}</div>}
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
  const debounceRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const firstRun = useRef(true);

  useEffect(() => setDraft(settings), [settings]);

  useEffect(() => {
    api.listJavas().then(setJavas).catch(() => {});
    api.getAppInfo().then(setAppInfo).catch(() => {});
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

  const parseNum = (value: string, fallback: number) => {
    const n = Number(value);
    return Number.isFinite(n) && n > 0 ? Math.round(n) : fallback;
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <PageHeader
        title="Settings"
        subtitle="Defaults for every instance. Changes save automatically."
        actions={
          <span
            className={cn(
              "inline-flex items-center gap-1.5 text-xs font-medium text-ok transition-opacity duration-300",
              saved ? "opacity-100" : "opacity-0",
            )}
          >
            <Check className="size-3.5" />
            Saved
          </span>
        }
      />

      <div className="min-h-0 flex-1 overflow-y-auto p-6">
        <div className="mx-auto flex max-w-2xl flex-col gap-7">
          <Section
            title="Game defaults"
            description="Applied to every launch unless an instance overrides them."
          >
            <Row label="Minimum memory" hint="JVM initial heap">
              <input
                type="number"
                value={draft.min_memory_mb}
                onChange={(e) => set({ min_memory_mb: parseNum(e.target.value, 512) })}
                className={cn(inputCls, "w-24 text-right")}
              />
              <span className="text-xs text-content-faint">MB</span>
            </Row>
            <Row label="Maximum memory" hint="JVM heap ceiling">
              <input
                type="number"
                value={draft.max_memory_mb}
                onChange={(e) => set({ max_memory_mb: parseNum(e.target.value, 2048) })}
                className={cn(inputCls, "w-24 text-right")}
              />
              <span className="text-xs text-content-faint">MB</span>
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
            >
              <div className="w-80">
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
              <Row label="Custom path" hint="path to a java executable">
                <input
                  value={draft.java_path ?? ""}
                  onChange={(e) => set({ java_path: e.target.value || null })}
                  placeholder="/path/to/bin/java"
                  className={cn(inputCls, "w-80 font-mono text-xs")}
                />
              </Row>
            )}
          </Section>

          <Section title="Downloads">
            <Row label="Concurrent downloads" hint="parallel files during installs">
              <input
                type="number"
                value={draft.concurrent_downloads}
                onChange={(e) =>
                  set({ concurrent_downloads: parseNum(e.target.value, 16) })
                }
                className={cn(inputCls, "w-24 text-right")}
              />
            </Row>
          </Section>

          <Section
            title="Integrations"
            description="Modrinth works out of the box. CurseForge requires a personal key because their API is keyed per application."
          >
            <Row
              label="CurseForge API key"
              hint={draft.curseforge_api_key ? "key set" : "not set, CurseForge search disabled"}
            >
              <input
                type="password"
                value={draft.curseforge_api_key ?? ""}
                onChange={(e) => set({ curseforge_api_key: e.target.value || null })}
                placeholder="paste your key"
                className={cn(inputCls, "w-64")}
              />
              <button
                onClick={() => openUrl("https://console.curseforge.com/")}
                title="Get a key"
                className="grid size-9 place-items-center rounded-lg border border-border bg-surface-2 text-content-faint transition-colors hover:bg-surface-3 hover:text-content"
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
              <div className="w-44">
                <Select
                  value={logConfig?.level ?? draft.log_level}
                  options={logConfig?.levels ?? ["error", "warn", "info", "debug", "trace"]}
                  onChange={(level) => void setLogLevel(level as LogLevel)}
                />
              </div>
            </Row>
            <Row label="Log file" hint={logConfig?.file ?? "resolving"}>
              <button
                onClick={() => setView("logs")}
                className="inline-flex items-center gap-2 rounded-lg border border-border bg-surface-2 px-3 py-2 text-xs font-medium text-content-muted transition-colors hover:bg-surface-3 hover:text-content"
              >
                <ScrollText className="size-3.5" />
                View logs
              </button>
              <button
                onClick={() => logConfig && openPath(logConfig.directory)}
                className="inline-flex items-center gap-2 rounded-lg border border-border bg-surface-2 px-3 py-2 text-xs font-medium text-content-muted transition-colors hover:bg-surface-3 hover:text-content"
              >
                <FolderOpen className="size-3.5" />
                Open folder
              </button>
            </Row>
          </Section>

          <Section title="Storage">
            <Row
              label="Data directory"
              hint={appInfo?.data_dir ?? "resolving"}
            >
              <button
                onClick={() => appInfo && openPath(appInfo.data_dir)}
                className="inline-flex items-center gap-2 rounded-lg border border-border bg-surface-2 px-3 py-2 text-xs font-medium text-content-muted transition-colors hover:bg-surface-3 hover:text-content"
              >
                <FolderOpen className="size-3.5" />
                Open folder
              </button>
            </Row>
          </Section>

          <div className="flex items-center justify-center gap-4 pb-2 pt-1 text-xs text-content-faint">
            <span className="inline-flex items-center gap-1.5">
              <Database className="size-3.5" />
              basalt.db
            </span>
            <span className="inline-flex items-center gap-1.5">
              <Coffee className="size-3.5" />
              {javas.length} java runtime{javas.length === 1 ? "" : "s"}
            </span>
            <span>Basalt {appInfo?.version ?? ""}</span>
          </div>
        </div>
      </div>
    </div>
  );
}
