import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Check,
  CircleStop,
  Copy,
  FolderOpen,
  Search,
  Share2,
  Trash2,
  TriangleAlert,
} from "lucide-react";

import { DiagnosisPanel } from "../components/logs/DiagnosisPanel";
import { FileLogPanel } from "../components/logs/FileLogPanel";
import { GameLogPanel } from "../components/logs/GameLogPanel";
import {
  LauncherLogPanel,
  filterRecords,
  recordsToText,
} from "../components/logs/LauncherLogPanel";
import { LogSourceTree, type LogSelection } from "../components/logs/LogSourceTree";
import { ShareLogModal } from "../components/ShareLogModal";
import type { LineLevel } from "../components/logs/lines";
import { api } from "../lib/api";
import { useUptime } from "../lib/useUptime";
import { Select } from "../components/Select";
import { cn } from "../lib/cn";
import { log } from "../lib/log";
import { notifyRemoved } from "../lib/notify";
import { openFolder } from "../lib/reveal";
import type { LogLevel, LogSearch, LogSource } from "../lib/types";
import { useStore } from "../store";

const MAX_ROWS = 1500;

const LEVEL_LABELS: Record<LogLevel | "all", string> = {
  all: "All",
  error: "Error",
  warn: "Warn",
  info: "Info",
  debug: "Debug",
  trace: "Trace",
};

const SOURCE_LABELS: Record<LogSource | "all", string> = {
  all: "All",
  backend: "Backend",
  frontend: "Frontend",
};

const LINE_LEVEL_LABELS: Record<LineLevel | "all", string> = {
  all: "All",
  error: "Error",
  warn: "Warn",
  info: "Info",
  debug: "Debug",
};

function keyOf<T extends string>(labels: Record<T, string>, label: string, fallback: T): T {
  return (Object.keys(labels) as T[]).find((key) => labels[key] === label) ?? fallback;
}


function ToolButton({
  onClick,
  title,
  children,
}: {
  onClick: () => void;
  title: string;
  children: React.ReactNode;
}) {
  return (
    <button
      onClick={onClick}
      title={title}
      className="inline-flex shrink-0 items-center gap-1.5 rounded-lg border border-border bg-surface-2 px-2.5 py-1.5 text-[11px] font-medium text-content-muted transition-colors hover:bg-surface-3 hover:text-content"
    >
      {children}
    </button>
  );
}

export function LogsView() {
  const records = useStore((s) => s.logRecords);
  const config = useStore((s) => s.logConfig);
  const clearLogs = useStore((s) => s.clearLogs);
  const refreshLogs = useStore((s) => s.refreshLogs);
  const running = useStore((s) => s.running);
  const logsMap = useStore((s) => s.logs);
  const instances = useStore((s) => s.instances);
  const killInstance = useStore((s) => s.killInstance);
  const closeRunning = useStore((s) => s.closeRunning);
  const logsTab = useStore((s) => s.logsTab);
  const setLogsTab = useStore((s) => s.setLogsTab);
  const activeRunningId = useStore((s) => s.activeRunningId);

  const [selection, setSelection] = useState<LogSelection>({ kind: "launcher" });
  const [minLevel, setMinLevel] = useState<LogLevel | "all">("info");
  const [lineLevel, setLineLevel] = useState<LineLevel | "all">("all");
  const [fileResult, setFileResult] = useState<LogSearch | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [treeVersion, setTreeVersion] = useState(0);
  const [source, setSource] = useState<LogSource | "all">("all");
  const [query, setQuery] = useState("");
  const [autoscroll, setAutoscroll] = useState(true);
  const [copied, setCopied] = useState(false);
  const [sharing, setSharing] = useState(false);

  useEffect(() => {
    if (!config) void refreshLogs();
  }, [config, refreshLogs]);

  useEffect(() => {
    if (logsTab === "game" && activeRunningId) {
      setSelection({ kind: "run", runningId: activeRunningId });
    }
  }, [logsTab, activeRunningId]);

  const select = (next: LogSelection) => {
    setSelection(next);
    setQuery("");
    setConfirmDelete(false);
    if (next.kind === "file") setFileResult(null);
    setLogsTab(next.kind === "run" ? "game" : next.kind === "file" ? "files" : "launcher");
  };

  const runs = useMemo(
    () => Object.values(running).sort((a, b) => b.started_at - a.started_at),
    [running],
  );

  const issues = useMemo(
    () => records.filter((r) => r.level === "error" || r.level === "warn").length,
    [records],
  );

  const filtered = useMemo(
    () => filterRecords(records, minLevel, source, query),
    [records, minLevel, source, query],
  );
  const visible = filtered.length > MAX_ROWS ? filtered.slice(-MAX_ROWS) : filtered;

  const run = selection.kind === "run" ? running[selection.runningId] : undefined;
  const runInstance = instances.find((i) => i.id === run?.instance_id);
  const live = run?.state === "running";
  const uptime = useUptime(run?.started_at ?? 0, !!live);

  const fileInstance =
    selection.kind === "file" ? instances.find((i) => i.id === selection.instanceId) : undefined;

  const runLines = useMemo(() => {
    if (selection.kind !== "run") return [];
    const all = (logsMap[selection.runningId] ?? []).map((entry) => entry.line);
    const needle = query.trim().toLowerCase();
    return needle ? all.filter((line) => line.toLowerCase().includes(needle)) : all;
  }, [selection, logsMap, query]);

  const copyCount =
    selection.kind === "launcher"
      ? filtered.length
      : selection.kind === "run"
        ? runLines.length
        : (fileResult?.hits.length ?? 0);

  const narrowed =
    selection.kind === "launcher"
      ? !!query.trim() || minLevel !== "all" || source !== "all"
      : !!query.trim() || lineLevel !== "all";

  const copy = async () => {
    const text =
      selection.kind === "launcher"
        ? recordsToText(filtered)
        : selection.kind === "run"
          ? runLines.join("\n")
          : (fileResult?.hits ?? []).map((hit) => hit.line).join("\n");
    if (!text) return;
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 1600);
    } catch (e) {
      log.warn("logs", `could not copy to clipboard: ${String(e)}`);
    }
  };

  const removeFile = async () => {
    if (selection.kind !== "file") return;
    try {
      await api.deleteInstanceLog(selection.instanceId, selection.name, selection.crash);
      notifyRemoved(`Deleted ${selection.name}`, fileInstance?.name);
      setTreeVersion((v) => v + 1);
      select({ kind: "launcher" });
    } catch (e) {
      log.warn("logs", `could not delete the log file: ${String(e)}`);
    }
  };

  const shareTitle =
    selection.kind === "file"
      ? selection.name
      : selection.kind === "run"
        ? `${runInstance?.name ?? "This run"} output`
        : "The launcher log";

  const loadShareText = useCallback(() => {
    if (selection.kind === "file")
      return api.redactInstanceLog(selection.instanceId, selection.name, selection.crash);
    const text =
      selection.kind === "run"
        ? (logsMap[selection.runningId] ?? []).map((entry) => entry.line).join("\n")
        : recordsToText(records);
    return api.redactText(text);
  }, [selection, logsMap, records]);

  const onFileResult = useCallback((search: LogSearch | null) => setFileResult(search), []);

  return (
    <div className="flex min-h-0 flex-1">
      <LogSourceTree
        selection={selection}
        onSelect={select}
        runs={runs}
        issues={issues}
        version={treeVersion}
      />

      <div className="flex min-h-0 min-w-0 flex-1 flex-col">
        <div className="flex flex-wrap items-center gap-2 border-b border-border-soft px-4 py-2">
          <div className="flex min-w-0 shrink-0 items-center gap-2 pr-1">
            <span className="truncate font-display text-[13px] font-semibold text-content">
              {selection.kind === "launcher"
                ? "Launcher"
                : selection.kind === "run"
                  ? (runInstance?.name ?? "Instance")
                  : selection.name}
            </span>
            {selection.kind === "run" && run && (
              <>
                <span
                  className={cn(
                    "shrink-0 rounded border px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide",
                    run.state === "running"
                      ? "border-ok/30 bg-ok/10 text-ok"
                      : run.state === "crashed"
                        ? "border-danger/30 bg-danger/10 text-danger"
                        : "border-border bg-surface-2 text-content-muted",
                  )}
                >
                  {run.state}
                  {run.exit_code != null && run.state !== "running" ? ` (${run.exit_code})` : ""}
                </span>
                <span className="shrink-0 text-[11px] text-content-faint">{uptime}</span>
              </>
            )}
            {selection.kind === "file" && fileInstance && (
              <span className="shrink-0 text-[11px] text-content-faint">{fileInstance.name}</span>
            )}
        </div>

        <div className="relative min-w-56 max-w-md flex-1">
          <Search className="pointer-events-none absolute left-2.5 top-1/2 size-3.5 -translate-y-1/2 text-content-faint" />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Filter lines"
            className="w-full rounded-lg border border-border bg-void py-1.5 pl-8 pr-2.5 text-xs text-content outline-none transition-colors placeholder:text-content-faint focus:border-(--accent)"
          />
        </div>

        {selection.kind === "launcher" && (
          <>
            <div className="w-36 shrink-0">
              <Select
                compact
                label="Level"
                value={LEVEL_LABELS[minLevel]}
                options={Object.values(LEVEL_LABELS)}
                onChange={(label) => setMinLevel(keyOf(LEVEL_LABELS, label, "all"))}
              />
            </div>

            <div className="w-40 shrink-0">
              <Select
                compact
                label="Source"
                value={SOURCE_LABELS[source]}
                options={Object.values(SOURCE_LABELS)}
                onChange={(label) => setSource(keyOf(SOURCE_LABELS, label, "all"))}
              />
            </div>
          </>
        )}

        {selection.kind !== "launcher" && (
          <div className="w-36 shrink-0">
            <Select
              compact
              label="Level"
              value={LINE_LEVEL_LABELS[lineLevel]}
              options={Object.values(LINE_LEVEL_LABELS)}
              onChange={(label) => setLineLevel(keyOf(LINE_LEVEL_LABELS, label, "all"))}
            />
          </div>
        )}

        <span className="flex-1" />

        {selection.kind !== "file" && (
          <button
            onClick={() => setAutoscroll((v) => !v)}
            className={cn(
              "shrink-0 rounded-lg border px-2.5 py-1.5 text-[11px] font-medium transition-colors",
              autoscroll
                ? "border-lava/40 bg-lava/10 text-ember"
                : "border-border bg-surface-2 text-content-muted hover:text-content",
            )}
          >
            Autoscroll
          </button>
        )}

        <ToolButton onClick={() => setSharing(true)} title="Upload a redacted copy to mclo.gs">
          <Share2 className="size-3.5" />
          Share
        </ToolButton>

        <ToolButton
          onClick={copy}
          title={
            narrowed
              ? `Copy the ${copyCount} lines matching this filter`
              : `Copy all ${copyCount} lines`
          }
        >
          {copied ? <Check className="size-3.5 text-ok" /> : <Copy className="size-3.5" />}
          {copied ? "Copied" : narrowed ? `Copy ${copyCount}` : "Copy all"}
        </ToolButton>

        {selection.kind === "launcher" && (
          <>
            {config && (
              <ToolButton onClick={() => openFolder(config.directory)} title={config.file}>
                <FolderOpen className="size-3.5" />
                Folder
              </ToolButton>
            )}
            <ToolButton onClick={() => void clearLogs()} title="Clear the log view">
              <Trash2 className="size-3.5" />
              Clear
            </ToolButton>
          </>
        )}

        {selection.kind === "run" && run && (
          <>
            {live ? (
              <button
                onClick={() => killInstance(run.running_id)}
                className="inline-flex shrink-0 items-center gap-1.5 rounded-lg border border-danger/40 bg-danger/10 px-2.5 py-1.5 text-[11px] font-semibold text-danger transition-colors hover:bg-danger/20"
              >
                <CircleStop className="size-3.5" />
                Stop
              </button>
            ) : (
              <ToolButton
                onClick={() => {
                  void closeRunning(run.running_id);
                  select({ kind: "launcher" });
                }}
                title="Drop this run from the list"
              >
                <Trash2 className="size-3.5" />
                Discard
              </ToolButton>
            )}
          </>
        )}

        {selection.kind === "file" && fileInstance && (
          <>
            <ToolButton
              onClick={() =>
                openFolder(`${fileInstance.dir}/${selection.crash ? "crash-reports" : "logs"}`)
              }
              title="Open the folder"
            >
              <FolderOpen className="size-3.5" />
              Folder
            </ToolButton>
            {confirmDelete ? (
              <button
                onClick={() => void removeFile()}
                onBlur={() => setConfirmDelete(false)}
                autoFocus
                title="This cannot be undone"
                className="inline-flex shrink-0 items-center gap-1.5 rounded-lg border border-danger/40 bg-danger/10 px-2.5 py-1.5 text-[11px] font-semibold text-danger transition-colors hover:bg-danger/20"
              >
                <TriangleAlert className="size-3.5" />
                Delete for good
              </button>
            ) : (
              <ToolButton onClick={() => setConfirmDelete(true)} title="Delete this log file">
                <Trash2 className="size-3.5" />
                Delete
              </ToolButton>
            )}
          </>
        )}
        </div>

        {selection.kind === "run" && runInstance && run?.state === "crashed" && (
          <DiagnosisPanel instance={runInstance} />
        )}

        {selection.kind === "file" && fileInstance && (
          <DiagnosisPanel
            instance={fileInstance}
            logName={selection.name}
            crash={selection.crash}
          />
        )}

        {selection.kind === "launcher" && config?.env_override && (
          <div className="border-b border-border-soft bg-warn/5 px-4 py-1.5 text-[11px] text-warn">
            BASALT_LOG={config.env_override} overrides the capture level for this session.
          </div>
        )}

        {selection.kind === "launcher" && (
          <>
            <LauncherLogPanel
              visible={visible}
              hidden={filtered.length - visible.length}
              empty={records.length === 0}
              autoscroll={autoscroll}
              query={query}
            />
            <div className="flex items-center gap-4 border-t border-border-soft px-4 py-1.5 text-[11px] text-content-faint">
              <span>
                {filtered.length} of {records.length} lines
              </span>
              <span>capture {config?.level ?? "info"}</span>
            </div>
          </>
        )}

        {selection.kind === "run" &&
          (run ? (
            <GameLogPanel
              runningId={selection.runningId}
              query={query}
              minLevel={lineLevel}
              autoscroll={autoscroll}
            />
          ) : (
            <div className="grid flex-1 place-items-center bg-void text-sm text-content-muted">
              That run is gone.
            </div>
          ))}

        {selection.kind === "file" && (
          <FileLogPanel
            instanceId={selection.instanceId}
            name={selection.name}
            crash={selection.crash}
            query={query}
            minLevel={lineLevel}
            onResult={onFileResult}
          />
        )}
      </div>

      <ShareLogModal
        open={sharing}
        onClose={() => setSharing(false)}
        title={shareTitle}
        load={loadShareText}
      />
    </div>
  );
}
