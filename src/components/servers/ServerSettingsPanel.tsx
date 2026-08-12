import { useEffect, useState } from "react";
import { Check, ClipboardCopy, Loader2 } from "lucide-react";
import { toast } from "sonner";

import { api } from "../../lib/api";
import { cn } from "../../lib/cn";
import { flavorLabel, isNative, needsFlavorVersion } from "../../lib/servers";
import type { JavaInfo, Server, SystemStats } from "../../lib/types";
import { MemoryRange } from "../MemoryRange";
import { Select } from "../Select";
import { Toggle } from "../ui";
import { useStore } from "../../store";

const JAVA_AUTO = "Auto-detect";
const JAVA_CUSTOM = "Custom path";
const APPEND = "Append to defaults";
const REPLACE = "Replace defaults";
const MODES = [APPEND, REPLACE];

const CEILING_FALLBACK = 16384;
const JVM_ARGS_FILE = "user_jvm_args.txt";

const inputCls =
  "rounded-lg border border-border bg-void px-3 py-2 text-sm text-content outline-none transition-colors focus:border-(--accent)";

function SectionLabel({ children }: { children: React.ReactNode }) {
  return (
    <h2 className="font-pixel text-[10px] uppercase tracking-[0.28em] text-content-faint">
      {children}
    </h2>
  );
}

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="grid grid-cols-[minmax(0,14rem)_minmax(0,1fr)] items-center gap-4 border-b border-border-soft/50 py-2.5">
      <span className="text-[12px] text-content-muted">{label}</span>
      <div className="flex min-w-0 items-center gap-2">{children}</div>
    </div>
  );
}

export function ServerSettingsPanel({ server, live }: { server: Server; live: boolean }) {
  const refreshServers = useStore((s) => s.refreshServers);
  const software = useStore((s) => s.serverSoftware);
  const settings = useStore((s) => s.settings);

  const [name, setName] = useState(server.name);
  const [versionId, setVersionId] = useState(server.version_id);
  const [flavorVersion, setFlavorVersion] = useState(server.flavor_version);
  const [minMem, setMinMem] = useState(server.min_memory_mb?.toString() ?? "");
  const [maxMem, setMaxMem] = useState(server.max_memory_mb?.toString() ?? "");
  const [javaPath, setJavaPath] = useState(server.java_path ?? "");
  const [javaCustom, setJavaCustom] = useState(false);
  const [jvmArgs, setJvmArgs] = useState(server.jvm_args ?? "");
  const [jvmArgsMode, setJvmArgsMode] = useState(server.jvm_args_mode ?? "append");
  const [stopTimeout, setStopTimeout] = useState(server.stop_timeout_secs?.toString() ?? "");
  const [notes, setNotes] = useState(server.notes ?? "");

  const [javas, setJavas] = useState<JavaInfo[]>([]);
  const [stats, setStats] = useState<SystemStats | null>(null);
  const [gameVersions, setGameVersions] = useState<string[]>([]);
  const [builds, setBuilds] = useState<string[]>([]);
  const [buildsLoading, setBuildsLoading] = useState(false);
  const [command, setCommand] = useState<string | null>(null);
  const [commandError, setCommandError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [scriptMemory, setScriptMemory] = useState<[string | null, string | null] | null>(null);
  const [applying, setApplying] = useState(false);

  useEffect(() => {
    setName(server.name);
    setVersionId(server.version_id);
    setFlavorVersion(server.flavor_version);
    setMinMem(server.min_memory_mb?.toString() ?? "");
    setMaxMem(server.max_memory_mb?.toString() ?? "");
    setJavaPath(server.java_path ?? "");
    setJavaCustom(false);
    setJvmArgs(server.jvm_args ?? "");
    setJvmArgsMode(server.jvm_args_mode ?? "append");
    setStopTimeout(server.stop_timeout_secs?.toString() ?? "");
    setNotes(server.notes ?? "");
  }, [server]);

  useEffect(() => {
    api.listJavas().then(setJavas).catch(() => {});
    api.getSystemStats().then(setStats).catch(() => {});
    api
      .listVersions(false)
      .then((list) => setGameVersions(list.map((entry) => entry.id)))
      .catch(() => {});
  }, [server.id]);

  useEffect(() => {
    setBuilds([]);
    if (server.pack_project_id || !needsFlavorVersion(software, server.flavor)) return;
    let alive = true;
    setBuildsLoading(true);
    api
      .listServerFlavorVersions(server.flavor, versionId)
      .then((list) => alive && setBuilds(list))
      .catch(() => alive && setBuilds([]))
      .finally(() => alive && setBuildsLoading(false));
    return () => {
      alive = false;
    };
  }, [server.flavor, server.pack_project_id, versionId]);

  const loadCommand = () => {
    setCommandError(null);
    api
      .getServerLaunchCommand(server.id)
      .then((line) => {
        setCommand(line);
        setCommandError(null);
      })
      .catch((cause) => {
        setCommand(null);
        setCommandError(String(cause));
      });
  };

  useEffect(loadCommand, [server]);

  useEffect(() => {
    setScriptMemory(null);
    if (!server.launch_script) return;
    let alive = true;
    api
      .getServerScriptMemory(server.id)
      .then((found) => alive && setScriptMemory(found))
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, [server.id, server.launch_script]);

  const applyScriptMemory = async () => {
    setApplying(true);
    setError(null);
    try {
      await api.applyServerScriptMemory(server.id);
      setScriptMemory(await api.getServerScriptMemory(server.id));
      toast.success(`Added memory limits to ${JVM_ARGS_FILE}`);
    } catch (cause) {
      setError(String(cause));
      toast.error(`Could not update ${JVM_ARGS_FILE}`, { description: String(cause) });
    } finally {
      setApplying(false);
    }
  };

  const native = isNative(software, server.flavor);
  const packLocked = !!server.pack_project_id;
  const scripted = !!server.launch_script && !server.skip_launch_script;
  const canRunWithoutScript = !!server.launch_jar || server.launch_argfiles.length > 0;
  const sliderMin = Number(minMem) || settings?.server_min_memory_mb || 1024;
  const sliderMax = Number(maxMem) || settings?.server_max_memory_mb || 4096;
  const ceiling = stats?.total_memory_mb ?? CEILING_FALLBACK;
  const reinstall =
    !packLocked &&
    (versionId !== server.version_id ||
      (flavorVersion ?? null) !== (server.flavor_version ?? null));
  const origin = packLocked
    ? `${server.pack_provider === "modrinth" ? "Modrinth" : "CurseForge"} modpack`
    : server.import_source === "folder"
      ? "Imported folder"
      : server.import_source === "zip"
        ? "Imported zip"
        : "Manual setup";

  const save = async () => {
    setSaving(true);
    setError(null);
    try {
      await api.updateServerSettings(
        server.id,
        name.trim(),
        versionId,
        needsFlavorVersion(software, server.flavor) ? flavorVersion : null,
        minMem.trim() === "" ? null : Number(minMem),
        maxMem.trim() === "" ? null : Number(maxMem),
        javaPath.trim() || null,
        jvmArgs.trim() || null,
        jvmArgs.trim() ? jvmArgsMode : null,
        stopTimeout.trim() === "" ? null : Number(stopTimeout),
        notes.trim() || null,
      );
      await refreshServers();
      if (server.launch_script) {
        setScriptMemory(await api.getServerScriptMemory(server.id));
      }
    } catch (cause) {
      setError(String(cause));
    } finally {
      setSaving(false);
    }
  };

  const copyCommand = async () => {
    if (!command) return;
    await navigator.clipboard.writeText(command);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex items-center gap-3 px-8 py-2">
        {reinstall && (
          <span className="font-pixel text-[10px] uppercase tracking-[0.22em] text-warn">
            saving a version change means installing again
          </span>
        )}
        {!reinstall && live && (
          <span className="font-pixel text-[10px] uppercase tracking-[0.22em] text-content-faint">
            applies on the next start
          </span>
        )}
        <button
          onClick={() => void save()}
          disabled={saving || (live && reinstall)}
          title={live && reinstall ? "Stop the server before changing its version" : undefined}
          className="ml-auto inline-flex items-center gap-2 rounded-lg bg-(--accent) px-3.5 py-1.5 text-[12px] font-semibold text-black transition-opacity disabled:cursor-not-allowed disabled:opacity-45"
        >
          {saving && <Loader2 className="size-3.5 animate-spin" />}
          Save
        </button>
      </div>

      {error && (
        <div className="wrap-break-word border-y border-danger/30 bg-danger/10 px-8 py-2 text-[11px] text-danger">
          {error}
        </div>
      )}

      <div className="min-h-0 flex-1 overflow-y-auto px-8 pb-8">
        <SectionLabel>Server</SectionLabel>
        <div className="mb-8 mt-1">
          <Row label="Name">
            <input
              value={name}
              onChange={(event) => setName(event.target.value)}
              className={cn(inputCls, "w-full max-w-md")}
            />
          </Row>
          <Row label="Version">
            {packLocked ? (
              <span className="text-[12px] text-content-muted">
                {server.version_id || "unknown"}
                {server.flavor_version && ` · ${server.flavor_version}`}
                {scripted
                  ? ", taken from the pack's script"
                  : ", managed by the modpack"}
              </span>
            ) : native ? (
              <span className="text-[12px] text-content-muted">
                Nightly, whatever Minecraft version this build targets
              </span>
            ) : (
              <div className="w-44">
                <Select
                  compact
                  value={versionId}
                  options={
                    gameVersions.includes(versionId)
                      ? gameVersions
                      : [versionId, ...gameVersions]
                  }
                  onChange={(next) => {
                    setVersionId(next);
                    setFlavorVersion(null);
                  }}
                />
              </div>
            )}
            {!packLocked && needsFlavorVersion(software, server.flavor) && (
              <div className="w-56">
                <Select
                  compact
                  value={flavorVersion}
                  placeholder={
                    buildsLoading
                      ? "Loading builds"
                      : builds.length === 0
                        ? "Nothing published"
                        : `Pick a ${flavorLabel(software, server.flavor)} build`
                  }
                  options={builds.slice(0, 80)}
                  onChange={setFlavorVersion}
                />
              </div>
            )}
          </Row>
          <Row label="Origin">
            <span className="text-[12px] text-content-muted">{origin}</span>
          </Row>
          <Row label="Folder">
            <span className="wrap-break-word font-mono text-[11px] text-content-faint">
              {server.dir}
            </span>
          </Row>
          <Row label="Notes">
            <textarea
              value={notes}
              onChange={(event) => setNotes(event.target.value)}
              rows={2}
              className={cn(inputCls, "w-full max-w-2xl resize-y text-xs")}
            />
          </Row>
        </div>

        {!native && (
          <>
          <SectionLabel>Memory</SectionLabel>
          <div className="mb-8 mt-3 max-w-2xl">
            <div className="mb-2 flex items-end justify-between gap-4">
              <div>
                <div className="text-[11px] font-medium text-content-muted">Minimum</div>
                <div className="mt-1 flex items-center gap-1.5">
                  <input
                    type="number"
                    value={minMem}
                    onChange={(event) => setMinMem(event.target.value)}
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
                    onChange={(event) => setMaxMem(event.target.value)}
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
                ceiling={ceiling}
                available={stats?.available_memory_mb}
                onChange={(low, high) => {
                  setMinMem(String(low));
                  setMaxMem(String(high));
                }}
              />
            </div>
            <p className="mt-2 text-[11px] text-content-faint">
              Leave both empty to follow the server default in Settings
              {settings
                ? `, now ${settings.server_min_memory_mb} MB to ${settings.server_max_memory_mb} MB`
                : ""}
              .
            </p>

            {server.launch_script && (
              <div className="mt-3 flex flex-wrap items-center gap-x-3 gap-y-1.5 border-t border-border-soft/50 pt-3">
                <span className="text-[11px] text-content-muted">
                  {scriptMemory?.[0] || scriptMemory?.[1]
                    ? `${JVM_ARGS_FILE} asks for ${scriptMemory[0] ?? "no minimum"} to ${scriptMemory[1] ?? "no maximum"}`
                    : `${JVM_ARGS_FILE} does not declare memory limits`}
                </span>
                <button
                  onClick={() => void applyScriptMemory()}
                  disabled={applying}
                  className="inline-flex items-center gap-1.5 rounded-lg border border-border bg-surface-2 px-2.5 py-1 text-[11px] font-medium text-content transition-colors hover:bg-surface-3 disabled:opacity-50"
                >
                  {applying && <Loader2 className="size-3 animate-spin" />}
                  Put saved values in it
                </button>
              </div>
            )}
          </div>

          <SectionLabel>Java</SectionLabel>
          <div className="mb-8 mt-1">
            <Row label="Runtime">
              <div className="w-full max-w-md">
                <Select
                  compact
                  value={
                    javaCustom
                      ? JAVA_CUSTOM
                      : !javaPath
                        ? JAVA_AUTO
                        : javas.find((entry) => entry.path === javaPath)
                          ? `Java ${javas.find((entry) => entry.path === javaPath)!.major} · ${javaPath}`
                          : JAVA_CUSTOM
                  }
                  options={[
                    JAVA_AUTO,
                    ...javas.map((entry) => `Java ${entry.major} · ${entry.path}`),
                    JAVA_CUSTOM,
                  ]}
                  onChange={(choice) => {
                    if (choice === JAVA_AUTO) {
                      setJavaCustom(false);
                      setJavaPath("");
                      return;
                    }
                    if (choice === JAVA_CUSTOM) {
                      setJavaCustom(true);
                      return;
                    }
                    const picked = javas.find(
                      (entry) => `Java ${entry.major} · ${entry.path}` === choice,
                    );
                    if (picked) {
                      setJavaCustom(false);
                      setJavaPath(picked.path);
                    }
                  }}
                />
              </div>
            </Row>
            {(javaCustom || (!!javaPath && !javas.some((entry) => entry.path === javaPath))) && (
              <Row label="Custom path">
                <input
                  value={javaPath}
                  onChange={(event) => setJavaPath(event.target.value)}
                  placeholder="/path/to/bin/java"
                  className={cn(inputCls, "w-full max-w-md font-mono text-xs")}
                />
              </Row>
            )}
            <Row label="Arguments">
              <textarea
                value={jvmArgs}
                onChange={(event) => setJvmArgs(event.target.value)}
                rows={2}
                spellCheck={false}
                placeholder={settings?.server_jvm_args.trim() || "-XX:+UseG1GC -Dsome.flag=true"}
                className={cn(inputCls, "w-full max-w-md resize-y font-mono text-xs")}
              />
              <div className="w-44 shrink-0 self-start">
                <Select
                  compact
                  value={jvmArgsMode === "replace" ? REPLACE : APPEND}
                  options={MODES}
                  onChange={(choice) => setJvmArgsMode(choice === REPLACE ? "replace" : "append")}
                />
              </div>
            </Row>
          </div>
          </>
        )}

        {server.launch_script && (
          <>
            <SectionLabel>Start script</SectionLabel>
            <div className="mb-8 mt-1">
              <Row label={server.launch_script}>
                <Toggle
                  label="Use the script this server ships"
                  checked={!server.skip_launch_script}
                  disabled={!server.skip_launch_script && !canRunWithoutScript}
                  onChange={(next) =>
                    void api
                      .setServerLaunchScript(server.id, next)
                      .then(() => refreshServers())
                      .catch((cause) => setError(String(cause)))
                  }
                />
                <span className="text-[12px] text-content-muted">
                  {server.skip_launch_script
                    ? "Bootstrap complete, Basalt runs its own command"
                    : canRunWithoutScript
                      ? "On, Basalt runs this instead of its own command"
                      : "Required until the script installs the loader"}
                </span>
              </Row>
              <p className="mt-2 max-w-2xl text-[11px] leading-relaxed text-content-faint">
                Basalt uses this script only while the downloaded pack still needs to install its
                loader. It ignores installer Java processes and switches future launches to
                Basalt's own Java command only after it finds complete launch files. Packs whose
                script cannot be understood safely stay script-managed.
              </p>
            </div>
          </>
        )}

        <SectionLabel>Launch command</SectionLabel>
        <div className="mb-8 mt-3 max-w-4xl">
          {command ? (
            <div className="flex items-start gap-2">
              <code className="selectable min-w-0 flex-1 wrap-break-word rounded-lg border border-border-soft bg-surface-2/50 px-3 py-2.5 font-mono text-[11px] leading-relaxed text-content-muted">
                {command}
              </code>
              <button
                onClick={() => void copyCommand()}
                title="Copy the command"
                className="grid size-8 shrink-0 place-items-center rounded-lg border border-border bg-surface-2 text-content-muted transition-colors hover:bg-surface-3 hover:text-content"
              >
                {copied ? <Check className="size-3.5 text-ok" /> : <ClipboardCopy className="size-3.5" />}
              </button>
            </div>
          ) : (
            <p className="wrap-break-word text-[11px] text-content-faint">
              {commandError ?? "Resolving"}
            </p>
          )}
          <p className="mt-2 text-[11px] text-content-faint">
            Built from the settings above every time the server starts.
          </p>
        </div>

        <SectionLabel>Stopping</SectionLabel>
        <div className="mt-1">
          <Row label="Wait before killing">
            <input
              type="number"
              min={5}
              max={600}
              value={stopTimeout}
              placeholder={settings ? String(settings.server_stop_timeout_secs) : "60"}
              onChange={(event) => setStopTimeout(event.target.value)}
              className={cn(inputCls, "w-24 text-right tabular-nums")}
            />
            <span className="text-xs text-content-faint">
              seconds for stop to save the world first, empty follows Settings
            </span>
          </Row>
        </div>
      </div>
    </div>
  );
}
