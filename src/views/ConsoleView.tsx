import { useEffect, useRef, useState } from "react";
import { CircleStop, Terminal, X } from "lucide-react";

import { cn } from "../lib/cn";
import type { LogLine } from "../lib/types";
import { useStore } from "../store";

const PREFIX_RE = /^(\[\d{2}:\d{2}:\d{2}\]) (\[[^\]]+\]:?)\s?(.*)$/;

function levelOf(log: LogLine): "error" | "warn" | "debug" | "info" {
  const line = log.line;
  if (/\/(ERROR|FATAL)\]/.test(line)) return "error";
  if (/\/WARN\]/.test(line)) return "warn";
  if (/\/(DEBUG|TRACE)\]/.test(line)) return "debug";
  if (/^\s+at |^Caused by:|Exception|^\tat /.test(line)) return "error";
  if (log.stream === "stderr") return "warn";
  return "info";
}

const LEVEL_CLASS: Record<ReturnType<typeof levelOf>, string> = {
  error: "text-danger",
  warn: "text-warn",
  debug: "text-content-faint",
  info: "text-content-muted",
};

function ConsoleLine({ log }: { log: LogLine }) {
  const level = levelOf(log);
  const match = log.line.match(PREFIX_RE);

  if (!match) {
    return (
      <div className={cn("whitespace-pre-wrap break-all", LEVEL_CLASS[level])}>
        {log.line}
      </div>
    );
  }

  const [, time, tag, body] = match;
  return (
    <div className="whitespace-pre-wrap break-all">
      <span className="text-content-faint/70">{time} </span>
      <span
        className={cn(
          level === "error"
            ? "text-danger/80"
            : level === "warn"
              ? "text-warn/80"
              : "text-content-faint",
        )}
      >
        {tag}{" "}
      </span>
      <span className={cn(level === "info" ? "text-content" : LEVEL_CLASS[level])}>
        {body}
      </span>
    </div>
  );
}

function useUptime(startedAt: number, live: boolean) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!live) return;
    const id = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(id);
  }, [live]);
  const secs = Math.max(0, Math.floor(now / 1000) - startedAt);
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = secs % 60;
  return h > 0
    ? `${h}h ${m}m ${s}s`
    : m > 0
      ? `${m}m ${s}s`
      : `${s}s`;
}

export function ConsoleView() {
  const runningId = useStore((s) => s.activeRunningId);
  const info = useStore((s) => (runningId ? s.running[runningId] : undefined));
  const logs = useStore((s) => (runningId ? (s.logs[runningId] ?? []) : []));
  const instance = useStore((s) =>
    s.instances.find((i) => i.id === info?.instance_id),
  );
  const killInstance = useStore((s) => s.killInstance);
  const closeRunning = useStore((s) => s.closeRunning);

  const scrollRef = useRef<HTMLDivElement>(null);
  const [autoscroll, setAutoscroll] = useState(true);

  const live = info?.state === "running";
  const uptime = useUptime(info?.started_at ?? 0, live);

  useEffect(() => {
    if (autoscroll && scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [logs, autoscroll]);

  if (!runningId || !info) {
    return (
      <div className="grid flex-1 place-items-center text-sm text-content-muted">
        Nothing is running.
      </div>
    );
  }

  const stateColor =
    info.state === "running"
      ? "text-ok border-ok/30 bg-ok/10"
      : info.state === "crashed"
        ? "text-danger border-danger/30 bg-danger/10"
        : "text-content-muted border-border bg-surface-2";

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex items-center justify-between gap-4 border-b border-border-soft px-6 py-4">
        <div className="flex items-center gap-3">
          <div className="grid size-9 place-items-center rounded-lg bg-surface-2 text-content-muted">
            <Terminal className="size-4" />
          </div>
          <div>
            <div className="font-display font-semibold text-content">
              {instance?.name ?? "Instance"}
            </div>
            <div className="flex items-center gap-2 text-xs text-content-muted">
              <span
                className={cn(
                  "rounded border px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide",
                  stateColor,
                )}
              >
                {info.state}
                {info.exit_code != null && info.state !== "running"
                  ? ` (${info.exit_code})`
                  : ""}
              </span>
              <span>PID {info.pid}</span>
              <span>·</span>
              <span>{uptime}</span>
            </div>
          </div>
        </div>

        <div className="flex items-center gap-2">
          <button
            onClick={() => setAutoscroll((v) => !v)}
            className={cn(
              "rounded-lg border px-3 py-1.5 text-xs font-medium transition-colors",
              autoscroll
                ? "border-lava/40 bg-lava/10 text-ember"
                : "border-border bg-surface-2 text-content-muted hover:text-content",
            )}
          >
            Autoscroll
          </button>
          {live ? (
            <button
              onClick={() => killInstance(runningId)}
              className="inline-flex items-center gap-1.5 rounded-lg border border-danger/40 bg-danger/10 px-3 py-1.5 text-xs font-semibold text-danger hover:bg-danger/20"
            >
              <CircleStop className="size-3.5" />
              Stop
            </button>
          ) : (
            <button
              onClick={() => closeRunning(runningId)}
              className="inline-flex items-center gap-1.5 rounded-lg border border-border bg-surface-2 px-3 py-1.5 text-xs font-semibold text-content hover:bg-surface-3"
            >
              <X className="size-3.5" />
              Close
            </button>
          )}
        </div>
      </div>

      <div
        ref={scrollRef}
        className="min-h-0 flex-1 overflow-y-auto bg-base px-4 py-3 font-mono text-xs leading-relaxed"
      >
        {logs.length === 0 ? (
          <div className="py-10 text-center text-content-faint">Waiting for output…</div>
        ) : (
          logs.map((log, i) => <ConsoleLine key={i} log={log} />)
        )}
      </div>
    </div>
  );
}
