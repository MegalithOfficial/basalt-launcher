import { Fragment, useEffect, useMemo, useRef, useState } from "react";
import type { ReactNode } from "react";
import { openPath } from "@tauri-apps/plugin-opener";
import { Check, Copy, FolderOpen, Search, Trash2 } from "lucide-react";

import { PageHeader } from "../components/ui";
import { cn } from "../lib/cn";
import { log } from "../lib/log";
import type { LogLevel, LogRecord, LogSource } from "../lib/types";
import { useStore } from "../store";

const RANK: Record<LogLevel, number> = {
  error: 0,
  warn: 1,
  info: 2,
  debug: 3,
  trace: 4,
};

const LEVEL_ORDER: LogLevel[] = ["error", "warn", "info", "debug", "trace"];
const LEVEL_FILTERS: Array<{ id: LogLevel | "all"; label: string }> = [
  { id: "all", label: "All" },
  ...LEVEL_ORDER.map((level) => ({ id: level, label: level })),
];
const SOURCES: Array<{ id: LogSource | "all"; label: string }> = [
  { id: "all", label: "All" },
  { id: "backend", label: "Backend" },
  { id: "frontend", label: "Frontend" },
];

const MAX_ROWS = 1500;
const CRATE_PREFIX = "basalt_launcher_lib::";

function shortTarget(target: string) {
  if (target === "basalt_launcher_lib") return "launcher";
  return target.startsWith(CRATE_PREFIX) ? target.slice(CRATE_PREFIX.length) : target;
}

const LEVEL_TEXT: Record<LogLevel, string> = {
  error: "text-danger",
  warn: "text-warn",
  info: "text-content",
  debug: "text-content-muted",
  trace: "text-content-faint",
};

const LEVEL_PILL: Record<LogLevel, string> = {
  error: "border-danger/30 bg-danger/10 text-danger",
  warn: "border-warn/30 bg-warn/10 text-warn",
  info: "border-border bg-surface-2 text-content-muted",
  debug: "border-border-soft bg-surface-2 text-content-faint",
  trace: "border-border-soft bg-surface-2 text-content-faint",
};

function formatTime(ts: number) {
  const date = new Date(ts);
  const pad = (n: number, width = 2) => String(n).padStart(width, "0");
  return `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(
    date.getSeconds(),
  )}.${pad(date.getMilliseconds(), 3)}`;
}

function haystack(record: LogRecord) {
  return [
    record.message,
    record.target,
    record.span ?? "",
    ...Object.entries(record.fields).map(([key, value]) => `${key}=${value}`),
  ]
    .join(" ")
    .toLowerCase();
}

function toText(records: LogRecord[]) {
  return records
    .map((r) => {
      const fields = Object.entries(r.fields)
        .map(([key, value]) => ` ${key}=${value}`)
        .join("");
      const span = r.span ? ` [${r.span}]` : "";
      return `${new Date(r.ts).toISOString()} ${r.level.toUpperCase().padEnd(5)} ${
        r.target
      }${span} ${r.message}${fields}`;
    })
    .join("\n");
}

function plural(count: number, noun: string) {
  return `${count} ${noun}${count === 1 ? "" : "s"}`;
}

function Dot() {
  return <span className="text-content-faint/50">·</span>;
}

function Chip({
  active,
  onClick,
  children,
}: {
  active: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "rounded-md px-2.5 py-1 text-xs font-medium capitalize transition-colors",
        active
          ? "bg-surface-3 text-content"
          : "text-content-faint hover:text-content-muted",
      )}
    >
      {children}
    </button>
  );
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
      className="inline-flex items-center gap-1.5 rounded-lg border border-border bg-surface-2 px-2.5 py-1.5 text-xs font-medium text-content-muted transition-colors hover:bg-surface-3 hover:text-content"
    >
      {children}
    </button>
  );
}

function LogRow({ record }: { record: LogRecord }) {
  const fields = Object.entries(record.fields);
  return (
    <div className="flex gap-3 border-b border-border-soft/50 px-4 py-1 hover:bg-surface/60">
      <span className="shrink-0 self-start tabular-nums text-content-faint/70">
        {formatTime(record.ts)}
      </span>
      <span
        className={cn(
          "mt-0.5 w-12 shrink-0 self-start rounded border px-1 text-center text-[10px] font-semibold uppercase leading-4 tracking-wide",
          LEVEL_PILL[record.level],
        )}
      >
        {record.level}
      </span>
      <span
        className="w-36 shrink-0 self-start break-words text-content-faint"
        title={record.span ? `${record.target} · ${record.span}` : record.target}
      >
        {shortTarget(record.target)}
      </span>
      <div className="min-w-0 flex-1">
        <span className={cn("whitespace-pre-wrap break-words", LEVEL_TEXT[record.level])}>
          {record.message}
        </span>
        {fields.length > 0 && (
          <span className="ml-2 break-words text-content-faint">
            {fields.map(([key, value]) => (
              <span key={key} className="mr-2">
                <span className="text-content-faint/60">{key}=</span>
                {value}
              </span>
            ))}
          </span>
        )}
        {record.span && (
          <div className="break-words text-[11px] text-content-faint/60">{record.span}</div>
        )}
      </div>
    </div>
  );
}

export function LogsView() {
  const records = useStore((s) => s.logRecords);
  const config = useStore((s) => s.logConfig);
  const clearLogs = useStore((s) => s.clearLogs);
  const refreshLogs = useStore((s) => s.refreshLogs);

  const [minLevel, setMinLevel] = useState<LogLevel | "all">("info");
  const [source, setSource] = useState<LogSource | "all">("all");
  const [query, setQuery] = useState("");
  const [autoscroll, setAutoscroll] = useState(true);
  const [copied, setCopied] = useState(false);

  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!config) void refreshLogs();
  }, [config, refreshLogs]);

  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    const ceiling = minLevel === "all" ? Infinity : RANK[minLevel];
    return records.filter((record) => {
      if (RANK[record.level] > ceiling) return false;
      if (source !== "all" && record.source !== source) return false;
      if (needle && !haystack(record).includes(needle)) return false;
      return true;
    });
  }, [records, minLevel, source, query]);

  const visible = useMemo(
    () => (filtered.length > MAX_ROWS ? filtered.slice(-MAX_ROWS) : filtered),
    [filtered],
  );

  const counts = useMemo(() => {
    let errors = 0;
    let warnings = 0;
    for (const record of records) {
      if (record.level === "error") errors += 1;
      else if (record.level === "warn") warnings += 1;
    }
    return { errors, warnings };
  }, [records]);

  const summary = useMemo(() => {
    const parts: ReactNode[] = [];
    if (counts.errors > 0) {
      parts.push(
        <span className="font-medium text-danger">{plural(counts.errors, "error")}</span>,
      );
    }
    if (counts.warnings > 0) {
      parts.push(
        <span className="font-medium text-warn">{plural(counts.warnings, "warning")}</span>,
      );
    }
    if (parts.length === 0) {
      parts.push(
        <span>
          {records.length === 0 ? "Waiting for the first line" : "No errors or warnings"}
        </span>,
      );
    }
    if (config) {
      const name = config.file.split(/[\\/]/).pop() ?? config.file;
      parts.push(
        <button
          onClick={() => void openPath(config.directory)}
          title={config.file}
          className="inline-flex items-center gap-1.5 font-mono text-xs text-content-faint transition-colors hover:text-content"
        >
          <FolderOpen className="size-3.5" />
          {name}
        </button>,
      );
    }
    return parts;
  }, [counts, records.length, config]);

  useEffect(() => {
    if (autoscroll && scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [visible, autoscroll]);

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(toText(filtered));
      setCopied(true);
      setTimeout(() => setCopied(false), 1600);
    } catch (e) {
      log.warn("logs", `could not copy to clipboard: ${String(e)}`);
    }
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <PageHeader
        title="Logs"
        subtitle={
          <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
            {summary.map((part, i) => (
              <Fragment key={i}>
                {i > 0 && <Dot />}
                {part}
              </Fragment>
            ))}
          </div>
        }
        actions={
          <div className="flex items-center gap-2">
            <ToolButton onClick={copy} title="Copy the filtered lines">
              {copied ? (
                <Check className="size-3.5 text-ok" />
              ) : (
                <Copy className="size-3.5" />
              )}
              {copied ? "Copied" : "Copy"}
            </ToolButton>
            <ToolButton onClick={() => void clearLogs()} title="Clear the log view">
              <Trash2 className="size-3.5" />
              Clear
            </ToolButton>
          </div>
        }
      />

      <div className="flex flex-wrap items-center gap-3 border-b border-border-soft px-6 py-3">
        <div
          role="group"
          aria-label="Level filter"
          className="flex items-center gap-0.5 rounded-lg border border-border-soft bg-surface-2/60 p-0.5"
        >
          {LEVEL_FILTERS.map((option) => (
            <Chip
              key={option.id}
              active={minLevel === option.id}
              onClick={() => setMinLevel(option.id)}
            >
              {option.label}
            </Chip>
          ))}
        </div>

        <div
          role="group"
          aria-label="Source filter"
          className="flex items-center gap-0.5 rounded-lg border border-border-soft bg-surface-2/60 p-0.5"
        >
          {SOURCES.map((option) => (
            <Chip
              key={option.id}
              active={source === option.id}
              onClick={() => setSource(option.id)}
            >
              {option.label}
            </Chip>
          ))}
        </div>

        <div className="relative min-w-56 flex-1">
          <Search className="pointer-events-none absolute left-3 top-1/2 size-3.5 -translate-y-1/2 text-content-faint" />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Filter by message, target or field"
            className="w-full rounded-lg border border-border bg-base py-2 pl-9 pr-3 text-sm text-content outline-none transition-colors placeholder:text-content-faint focus:border-[var(--accent)]"
          />
        </div>

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

      </div>

      {config?.env_override && (
        <div className="border-b border-border-soft bg-warn/5 px-6 py-2 text-xs text-warn">
          BASALT_LOG={config.env_override} overrides the capture level for this session.
        </div>
      )}

      <div
        ref={scrollRef}
        className="min-h-0 flex-1 overflow-y-auto bg-base font-mono text-xs leading-relaxed"
      >
        {visible.length === 0 ? (
          <div className="py-16 text-center text-content-faint">
            {records.length === 0 ? "Nothing logged yet." : "No lines match this filter."}
          </div>
        ) : (
          <>
            {filtered.length > visible.length && (
              <div className="px-4 py-2 text-content-faint/70">
                {filtered.length - visible.length} older lines hidden. Narrow the filter or
                open the log file.
              </div>
            )}
            {visible.map((record) => (
              <LogRow key={record.seq} record={record} />
            ))}
          </>
        )}
      </div>

      <div className="flex items-center gap-4 border-t border-border-soft px-6 py-2 text-xs text-content-faint">
        <span>
          {filtered.length} of {records.length} lines
        </span>
        <span>capture {config?.level ?? "info"}</span>
      </div>
    </div>
  );
}
