import { useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Loader2, RefreshCw, Save, TriangleAlert, X } from "lucide-react";

import { api } from "../../lib/api";
import { cn } from "../../lib/cn";
import { highlightLine, highlights, MAX_HIGHLIGHTED_LINES } from "../../lib/highlight";
import type { ServerText, TextProblem } from "../../lib/types";

export function ConfigEditor({
  serverId,
  file,
  onClose,
  onSaved,
  onReload,
}: {
  serverId: string;
  file: ServerText;
  onClose: () => void;
  onSaved: () => void;
  onReload: () => Promise<void>;
}) {
  const [text, setText] = useState(file.text);
  const [problem, setProblem] = useState<TextProblem | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const areaRef = useRef<HTMLTextAreaElement>(null);
  const overlayRef = useRef<HTMLDivElement>(null);
  const gutterRef = useRef<HTMLDivElement>(null);
  const dirtyRef = useRef(false);
  const reloadRef = useRef(onReload);

  useEffect(() => {
    setText(file.text);
    setProblem(null);
    setError(null);
  }, [file]);

  const lines = useMemo(() => text.split("\n"), [text]);
  const painted = highlights(file.kind) && lines.length <= MAX_HIGHLIGHTED_LINES;
  const dirty = text !== file.text;
  dirtyRef.current = dirty;
  reloadRef.current = onReload;

  useEffect(() => {
    let disposed = false;
    let stop: (() => void) | undefined;
    void listen<{ server_id: string; path: string }>("server:file-changed", (event) => {
      if (
        event.payload.server_id === serverId &&
        event.payload.path === file.path &&
        !dirtyRef.current
      ) {
        void reloadRef.current();
      }
    }).then((unlisten) => {
      if (disposed) unlisten();
      else stop = unlisten;
    });
    return () => {
      disposed = true;
      stop?.();
    };
  }, [file.path, serverId]);

  const save = async () => {
    setSaving(true);
    setError(null);
    try {
      const found = await api.writeServerFile(serverId, file.path, text);
      setProblem(found);
      if (!found) onSaved();
    } catch (cause) {
      setError(String(cause));
    } finally {
      setSaving(false);
    }
  };

  const check = async () => {
    try {
      setProblem(await api.checkServerFile(file.path, text));
    } catch {
      setProblem(null);
    }
  };

  const reload = async () => {
    setError(null);
    try {
      await onReload();
    } catch (cause) {
      setError(String(cause));
    }
  };

  const jumpTo = (line: number) => {
    const area = areaRef.current;
    if (!area) return;
    const offset = lines.slice(0, Math.max(0, line - 1)).reduce((sum, entry) => sum + entry.length + 1, 0);
    area.focus();
    area.setSelectionRange(offset, offset + (lines[line - 1]?.length ?? 0));
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex items-center gap-2 border-b border-border-soft px-8 py-2.5">
        <span className="min-w-0 flex-1 wrap-break-word font-mono text-[12px] text-content-muted">
          {file.path}
        </span>
        {dirty && (
          <span className="shrink-0 rounded-full bg-warn/15 px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wider text-warn">
            Unsaved
          </span>
        )}
        <button
          onClick={() => void reload()}
          disabled={dirty || saving}
          title={dirty ? "Save or discard your changes before reloading" : "Reload from disk"}
          className="grid size-7 shrink-0 place-items-center rounded-lg text-content-faint transition-colors hover:bg-surface-3 hover:text-content disabled:cursor-not-allowed disabled:opacity-40"
        >
          <RefreshCw className="size-3.5" />
        </button>
        <button
          onClick={() => void check()}
          className="shrink-0 rounded-lg border border-border bg-surface-2 px-2.5 py-1.5 text-[11px] font-medium text-content-muted transition-colors hover:bg-surface-3 hover:text-content"
        >
          Check
        </button>
        <button
          onClick={() => void save()}
          disabled={saving || !dirty}
          className="inline-flex shrink-0 items-center gap-1.5 rounded-lg bg-(--accent) px-3 py-1.5 text-[11px] font-semibold text-black transition-opacity disabled:cursor-not-allowed disabled:opacity-45"
        >
          {saving ? <Loader2 className="size-3.5 animate-spin" /> : <Save className="size-3.5" />}
          Save
        </button>
        <button
          onClick={onClose}
          title="Close the editor"
          className="grid size-7 shrink-0 place-items-center rounded-lg text-content-faint transition-colors hover:bg-surface-3 hover:text-content"
        >
          <X className="size-4" />
        </button>
      </div>

      {(problem || error) && (
        <div className="flex items-start gap-2 border-b border-danger/30 bg-danger/10 px-8 py-2 text-[11px] text-danger">
          <TriangleAlert className="mt-0.5 size-3.5 shrink-0" />
          <span className="min-w-0 flex-1 wrap-break-word">
            {problem ? `Line ${problem.line}, column ${problem.column}: ${problem.message}` : error}
          </span>
          {problem && (
            <button
              onClick={() => jumpTo(problem.line)}
              className="shrink-0 underline underline-offset-2"
            >
              Go to line
            </button>
          )}
        </div>
      )}

      <div className="relative min-h-0 flex-1 font-mono text-[12px] leading-[1.6]">
        <div
          ref={gutterRef}
          aria-hidden
          className="absolute inset-y-0 left-0 w-14 overflow-hidden border-r border-border-soft bg-surface-2/40 py-3 text-right text-content-faint"
        >
          {lines.map((_, index) => (
            <div key={index} className="px-2">
              {index + 1}
            </div>
          ))}
        </div>

        <div
          ref={overlayRef}
          aria-hidden
          className="pointer-events-none absolute inset-y-0 left-14 right-0 overflow-hidden whitespace-pre px-4 py-3"
        >
          {lines.map((line, index) => (
            <div key={index}>
              {painted ? (
                highlightLine(file.kind, line).map((token, at) => (
                  <span key={at} className={token.cls}>
                    {token.text}
                  </span>
                ))
              ) : (
                <span className="text-content-muted">{line}</span>
              )}
              {line === "" && "​"}
            </div>
          ))}
        </div>

        <textarea
          ref={areaRef}
          value={text}
          spellCheck={false}
          onChange={(event) => setText(event.target.value)}
          onScroll={(event) => {
            const node = event.currentTarget;
            if (overlayRef.current) {
              overlayRef.current.scrollTop = node.scrollTop;
              overlayRef.current.scrollLeft = node.scrollLeft;
            }
            if (gutterRef.current) gutterRef.current.scrollTop = node.scrollTop;
          }}
          onKeyDown={(event) => {
            if ((event.ctrlKey || event.metaKey) && event.key === "s") {
              event.preventDefault();
              void save();
            }
          }}
          className={cn(
            "absolute inset-y-0 left-14 right-0 resize-none overflow-auto whitespace-pre bg-transparent px-4 py-3 text-transparent caret-content outline-none",
          )}
        />
      </div>
    </div>
  );
}
