import { useEffect, useRef } from "react";

import { cn } from "../../lib/cn";
import type { LogLevel, LogRecord, LogSource } from "../../lib/types";
import { Mark, findRanges } from "./lines";

export const RANK: Record<LogLevel, number> = {
  error: 0,
  warn: 1,
  info: 2,
  debug: 3,
  trace: 4,
};

const CRATE_PREFIX = "basalt_launcher_lib::";

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
  info: "border-border bg-surface-2 text-content",
  debug: "border-debug/30 bg-debug/10 text-debug",
  trace: "border-trace/30 bg-trace/10 text-trace",
};

function shortTarget(target: string) {
  if (target === "basalt_launcher_lib") return "launcher";
  return target.startsWith(CRATE_PREFIX) ? target.slice(CRATE_PREFIX.length) : target;
}

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

export function filterRecords(
  records: LogRecord[],
  minLevel: LogLevel | "all",
  source: LogSource | "all",
  query: string,
) {
  const needle = query.trim().toLowerCase();
  const ceiling = minLevel === "all" ? Infinity : RANK[minLevel];
  return records.filter((record) => {
    if (RANK[record.level] > ceiling) return false;
    if (source !== "all" && record.source !== source) return false;
    if (needle && !haystack(record).includes(needle)) return false;
    return true;
  });
}

export function recordsToText(records: LogRecord[]) {
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

function LogRow({ record, query }: { record: LogRecord; query: string }) {
  const fields = Object.entries(record.fields);
  const target = shortTarget(record.target);
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
        className="w-36 shrink-0 self-start wrap-break-word text-content-faint"
        title={record.span ? `${record.target} · ${record.span}` : record.target}
      >
        <Mark text={target} ranges={findRanges(target, query)} />
      </span>
      <div className="min-w-0 flex-1">
        <span className={cn("whitespace-pre-wrap wrap-break-word", LEVEL_TEXT[record.level])}>
          <Mark text={record.message} ranges={findRanges(record.message, query)} />
        </span>
        {fields.length > 0 && (
          <span className="ml-2 wrap-break-word text-content-faint">
            {fields.map(([key, value]) => (
              <span key={key} className="mr-2">
                <span className="text-content-faint/60">{key}=</span>
                <Mark text={value} ranges={findRanges(value, query)} />
              </span>
            ))}
          </span>
        )}
        {record.span && (
          <div className="wrap-break-word text-[11px] text-content-faint/60">{record.span}</div>
        )}
      </div>
    </div>
  );
}

export function LauncherLogPanel({
  visible,
  hidden,
  empty,
  autoscroll,
  query,
}: {
  visible: LogRecord[];
  hidden: number;
  empty: boolean;
  autoscroll: boolean;
  query: string;
}) {
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (autoscroll && scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [visible, autoscroll]);

  return (
    <div
      ref={scrollRef}
      className="selectable min-h-0 flex-1 overflow-y-auto bg-void font-mono text-xs leading-relaxed"
    >
      {visible.length === 0 ? (
        <div className="py-16 text-center text-content-faint">
          {empty ? "Nothing logged yet." : "No lines match this filter."}
        </div>
      ) : (
        <>
          {hidden > 0 && (
            <div className="px-4 py-2 text-content-faint/70">
              {hidden} older lines hidden. Narrow the filter or open the log file.
            </div>
          )}
          {visible.map((record) => (
            <LogRow key={record.seq} record={record} query={query} />
          ))}
        </>
      )}
    </div>
  );
}
