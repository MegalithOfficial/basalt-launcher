import { useEffect, useMemo, useRef, useState } from "react";
import { ArrowDown, CornerDownLeft } from "lucide-react";

import { cn } from "../../lib/cn";
import { readConsoleLine } from "../../lib/servers";
import type { ConsoleLine } from "../../lib/types";
import { useStore } from "../../store";

const EMPTY: ConsoleLine[] = [];
const MAX_RENDERED_LINES = 1500;
const MAX_HISTORY = 100;
const STICK_THRESHOLD = 24;

export function ConsolePanel({
  serverId,
  live,
  attached,
}: {
  serverId: string;
  live: boolean;
  attached: boolean;
}) {
  const lines = useStore((s) => s.serverConsole[serverId] ?? EMPTY);
  const sendServerCommand = useStore((s) => s.sendServerCommand);

  const scrollRef = useRef<HTMLDivElement>(null);
  const history = useRef<string[]>([]);
  const [draft, setDraft] = useState("");
  const [cursor, setCursor] = useState(-1);
  const [atBottom, setAtBottom] = useState(true);
  const [unseen, setUnseen] = useState(0);
  const [error, setError] = useState<string | null>(null);

  const visible = useMemo(
    () => (lines.length > MAX_RENDERED_LINES ? lines.slice(-MAX_RENDERED_LINES) : lines),
    [lines],
  );

  useEffect(() => {
    const node = scrollRef.current;
    if (!node) return;
    if (atBottom) {
      node.scrollTop = node.scrollHeight;
      setUnseen(0);
    } else {
      setUnseen((count) => count + 1);
    }
  }, [lines.length, atBottom]);

  const jumpToLatest = () => {
    const node = scrollRef.current;
    if (!node) return;
    node.scrollTop = node.scrollHeight;
    setAtBottom(true);
    setUnseen(0);
  };

  const submit = async () => {
    const command = draft.trim();
    if (!command) return;
    history.current = [command, ...history.current.filter((entry) => entry !== command)].slice(
      0,
      MAX_HISTORY,
    );
    setDraft("");
    setCursor(-1);
    setError(null);
    try {
      await sendServerCommand(serverId, command);
    } catch (cause) {
      setError(String(cause));
    }
  };

  const walkHistory = (step: number) => {
    const next = cursor + step;
    if (next < 0) {
      setCursor(-1);
      setDraft("");
      return;
    }
    const entry = history.current[next];
    if (entry === undefined) return;
    setCursor(next);
    setDraft(entry);
  };

  return (
    <div className="flex min-h-0 min-w-0 flex-1 flex-col">
      <div className="relative min-h-0 flex-1">
        <div
          ref={scrollRef}
          onScroll={(event) => {
            const node = event.currentTarget;
            const bottom =
              node.scrollHeight - node.scrollTop - node.clientHeight < STICK_THRESHOLD;
            setAtBottom(bottom);
            if (bottom) setUnseen(0);
          }}
          className="selectable h-full overflow-auto px-8 py-4 font-mono text-[12px] leading-relaxed"
        >
          {lines.length === 0 ? (
            <div className="text-content-faint">
              {live ? "Waiting for the server to say something." : "The console is quiet."}
            </div>
          ) : (
            visible.map((entry, index) => {
              const parts = readConsoleLine(entry.stream, entry.line);
              return (
                <div key={index} className="whitespace-pre-wrap wrap-break-word">
                  {entry.stream === "input" ? (
                    <span className="text-content-faint">
                      <span className="mr-1.5 text-(--accent)">&gt;</span>
                      {parts.message}
                    </span>
                  ) : (
                    <>
                      {parts.time && <span className="text-content-faint">[{parts.time}] </span>}
                      {parts.source && (
                        <span className="text-content-faint">{parts.source} </span>
                      )}
                      <span
                        className={cn(
                          parts.level === "error"
                            ? "text-danger"
                            : parts.level === "warn"
                              ? "text-warn"
                              : "text-content",
                        )}
                      >
                        {parts.message}
                      </span>
                    </>
                  )}
                </div>
              );
            })
          )}
        </div>

        {!atBottom && (
          <button
            onClick={jumpToLatest}
            className="absolute bottom-3 left-1/2 inline-flex -translate-x-1/2 items-center gap-1.5 rounded-full border border-border bg-surface-3 px-3 py-1.5 text-[11px] font-medium text-content"
          >
            <ArrowDown className="size-3.5" />
            {unseen > 0 ? `Jump to latest (${unseen} new)` : "Jump to latest"}
          </button>
        )}
      </div>

      {error && (
        <div className="border-t border-danger/30 bg-danger/10 px-8 py-2 text-[11px] text-danger">
          {error}
        </div>
      )}

      <div className="flex items-center gap-2 border-t border-border-soft px-8 py-3">
        <input
          value={draft}
          disabled={!live || !attached}
          placeholder={
            !live
              ? "Start the server to send commands"
              : attached
                ? "Type a command, for example say hello"
                : "Basalt restarted while this server was running, so commands cannot reach it"
          }
          onChange={(event) => setDraft(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.preventDefault();
              void submit();
            } else if (event.key === "ArrowUp") {
              event.preventDefault();
              walkHistory(1);
            } else if (event.key === "ArrowDown") {
              event.preventDefault();
              walkHistory(-1);
            }
          }}
          className="min-w-0 flex-1 rounded-lg border border-border bg-surface-2 px-3 py-2 font-mono text-[12px] text-content placeholder:text-content-faint focus:border-(--accent) focus:outline-none disabled:cursor-not-allowed disabled:text-content-faint"
        />
        <button
          onClick={() => void submit()}
          disabled={!live || !attached || !draft.trim()}
          className="inline-flex shrink-0 items-center gap-1.5 rounded-lg border border-border bg-surface-2 px-3 py-2 text-[11px] font-medium text-content-muted transition-colors hover:bg-surface-3 hover:text-content disabled:cursor-not-allowed disabled:opacity-50"
        >
          <CornerDownLeft className="size-3.5" />
          Send
        </button>
      </div>
    </div>
  );
}
