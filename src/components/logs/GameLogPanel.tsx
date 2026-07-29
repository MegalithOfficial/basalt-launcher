import { useEffect, useMemo, useRef } from "react";

import { useStore } from "../../store";
import { LINE_RANK, OutputLine, findRanges, levelOfLine, type LineLevel } from "./lines";

const EMPTY_LOGS: never[] = [];

export function GameLogPanel({
  runningId,
  query,
  minLevel,
  autoscroll,
}: {
  runningId: string;
  query: string;
  minLevel: LineLevel | "all";
  autoscroll: boolean;
}) {
  const logs = useStore((s) => s.logs[runningId] ?? EMPTY_LOGS);
  const scrollRef = useRef<HTMLDivElement>(null);

  const visible = useMemo(() => {
    const needle = query.trim();
    const ceiling = minLevel === "all" ? Infinity : LINE_RANK[minLevel];

    let carried: LineLevel = "info";
    const levelled = logs.map((log) => {
      carried = levelOfLine(log.line, log.stream, carried);
      return { log, level: carried };
    });

    return levelled
      .filter((entry) => LINE_RANK[entry.level] <= ceiling)
      .map((entry) => ({ ...entry, ranges: needle ? findRanges(entry.log.line, needle) : [] }))
      .filter((entry) => !needle || entry.ranges.length > 0);
  }, [logs, query, minLevel]);

  useEffect(() => {
    if (autoscroll && scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [visible, autoscroll]);

  return (
    <div
      ref={scrollRef}
      className="min-h-0 flex-1 overflow-auto bg-base px-4 py-2.5 font-mono text-xs leading-relaxed"
    >
      {logs.length === 0 ? (
        <div className="py-10 text-center text-content-faint">Waiting for output</div>
      ) : visible.length === 0 ? (
        <div className="py-10 text-center text-content-faint">No lines match this filter.</div>
      ) : (
        visible.map((entry, i) => (
          <OutputLine key={i} line={entry.log.line} level={entry.level} ranges={entry.ranges} />
        ))
      )}
    </div>
  );
}
