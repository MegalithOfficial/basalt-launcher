import { useEffect, useState } from "react";

import { api } from "../../lib/api";
import type { LogSearch } from "../../lib/types";
import { OutputLine, type LineLevel } from "./lines";

export function FileLogPanel({
  instanceId,
  name,
  crash,
  query,
  minLevel,
  onResult,
}: {
  instanceId: string;
  name: string;
  crash: boolean;
  query: string;
  minLevel: LineLevel | "all";
  onResult: (search: LogSearch | null) => void;
}) {
  const [result, setResult] = useState<LogSearch | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    const timer = setTimeout(
      () => {
        setLoading(true);
        api
          .searchInstanceLog(instanceId, name, crash, query, minLevel === "all" ? null : minLevel)
          .then((search) => {
            if (cancelled) return;
            setResult(search);
            setError(null);
            onResult(search);
          })
          .catch((e) => {
            if (cancelled) return;
            setError(String(e));
            onResult(null);
          })
          .finally(() => {
            if (!cancelled) setLoading(false);
          });
      },
      query ? 140 : 0,
    );
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [instanceId, name, crash, query, minLevel, onResult]);

  const narrowed = !!query.trim() || minLevel !== "all";

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="selectable min-h-0 flex-1 overflow-auto bg-void px-4 py-2.5 font-mono text-xs leading-relaxed">
        {error && <div className="py-10 text-center text-danger">{error}</div>}
        {!error && !result && loading && (
          <div className="py-10 text-center text-content-faint">Reading</div>
        )}
        {!error && result && result.hits.length === 0 && (
          <div className="py-10 text-center text-content-faint">
            {result.total_lines === 0 ? "This file is empty." : "No lines match this filter."}
          </div>
        )}
        {!error &&
          result?.hits.map((hit) => (
            <OutputLine
              key={hit.number}
              line={hit.line}
              level={hit.level}
              ranges={hit.ranges}
            />
          ))}
      </div>

      {result && (
        <div className="flex items-center gap-4 border-t border-border-soft px-4 py-1.5 text-[11px] text-content-faint">
          <span>
            {narrowed
              ? `${result.matched_lines} of ${result.total_lines} lines match`
              : `${result.total_lines} lines`}
          </span>
          {result.truncated && <span>showing the first {result.hits.length}</span>}
        </div>
      )}
    </div>
  );
}
