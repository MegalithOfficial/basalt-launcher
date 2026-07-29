import { Fragment } from "react";

import { cn } from "../../lib/cn";

const PREFIX_RE = /^(\[\d{2}:\d{2}:\d{2}\]) (\[[^\]]+\]:?)\s?(.*)$/;

export type Range = [number, number];

export type LineLevel = "error" | "warn" | "debug" | "info";

const STAMP_RE = /^\[\d{2}:\d{2}:\d{2}\]/;
const FAULT_RE = /^\s+at |^Caused by:|^\s*Suppressed:|Exception/;

export function levelOfLine(
  line: string,
  stream?: string,
  previous: LineLevel = "info",
): LineLevel {
  if (STAMP_RE.test(line)) {
    if (/\/(ERROR|FATAL)\]/.test(line)) return "error";
    if (/\/WARN\]/.test(line)) return "warn";
    if (/\/(DEBUG|TRACE)\]/.test(line)) return "debug";
    return FAULT_RE.test(line) ? "error" : "info";
  }

  if (FAULT_RE.test(line)) return "error";
  if (stream === "stderr" && previous === "info") return "warn";
  return previous;
}

export const LINE_RANK: Record<LineLevel, number> = {
  error: 0,
  warn: 1,
  info: 2,
  debug: 3,
};

const LEVEL_CLASS: Record<LineLevel, string> = {
  error: "text-danger",
  warn: "text-warn",
  debug: "text-content-faint",
  info: "text-content-muted",
};

export function findRanges(text: string, query: string): Range[] {
  const needle = query.trim().toLowerCase();
  if (!needle) return [];
  const hay = text.toLowerCase();
  const ranges: Range[] = [];
  let from = 0;
  for (;;) {
    const at = hay.indexOf(needle, from);
    if (at === -1) break;
    ranges.push([at, at + needle.length]);
    from = at + needle.length;
  }
  return ranges;
}

export function Mark({
  text,
  ranges,
  offset = 0,
}: {
  text: string;
  ranges?: Range[];
  offset?: number;
}) {
  if (!ranges || ranges.length === 0) return <>{text}</>;

  const end = offset + text.length;
  const parts = [];
  let cursor = offset;

  for (const [start, stop] of ranges) {
    if (stop <= offset || start >= end) continue;
    const from = Math.max(start, cursor);
    const to = Math.min(stop, end);
    if (from > cursor) parts.push(text.slice(cursor - offset, from - offset));
    parts.push(
      <mark
        key={`${from}-${to}`}
        className="rounded-[3px] bg-(--accent)/35 px-px text-content ring-1 ring-inset ring-(--accent)/50"
      >
        {text.slice(from - offset, to - offset)}
      </mark>,
    );
    cursor = to;
  }
  if (cursor < end) parts.push(text.slice(cursor - offset));

  return (
    <>
      {parts.map((part, i) => (
        <Fragment key={i}>{part}</Fragment>
      ))}
    </>
  );
}

export function OutputLine({
  line,
  stream,
  ranges,
  level: given,
}: {
  line: string;
  stream?: string;
  ranges?: Range[];
  level?: LineLevel;
}) {
  const level = given ?? levelOfLine(line, stream);
  const match = line.match(PREFIX_RE);

  if (!match) {
    return (
      <div className={cn("whitespace-pre-wrap wrap-break-word", LEVEL_CLASS[level])}>
        <Mark text={line} ranges={ranges} />
      </div>
    );
  }

  const [, time, tag, body] = match;
  const tagAt = time.length + 1;
  const bodyAt = line.length - body.length;

  return (
    <div className="whitespace-pre-wrap wrap-break-word">
      <span className="text-content-faint/60">
        <Mark text={time} ranges={ranges} />{" "}
      </span>
      <span
        className={cn(
          level === "error"
            ? "text-danger/80"
            : level === "warn"
              ? "text-warn/80"
              : "text-content-faint",
        )}
      >
        <Mark text={tag} ranges={ranges} offset={tagAt} />{" "}
      </span>
      <span className={cn(level === "info" ? "text-content" : LEVEL_CLASS[level])}>
        <Mark text={body} ranges={ranges} offset={bodyAt} />
      </span>
    </div>
  );
}
