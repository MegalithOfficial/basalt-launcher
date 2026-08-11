import { useCallback, useEffect, useMemo, useState } from "react";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { FileUp, Loader2, RotateCw, Search, Trash2 } from "lucide-react";

import { api } from "../../lib/api";
import { cn } from "../../lib/cn";
import { formatBytes } from "../../lib/format";
import type { ContentItem, Server } from "../../lib/types";
import { ConfirmDialog } from "../ConfirmDialog";
import { Toggle } from "../ui";

export function ServerContentPanel({
  server,
  label,
  live,
}: {
  server: Server;
  label: string;
  live: boolean;
}) {
  const [items, setItems] = useState<ContentItem[]>([]);
  const [query, setQuery] = useState("");
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);
  const [doomed, setDoomed] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      setItems(await api.listServerContent(server.id));
      setError(null);
    } catch (cause) {
      setError(String(cause));
    } finally {
      setLoading(false);
    }
  }, [server.id]);

  useEffect(() => {
    void load();
  }, [load]);

  const shown = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return items;
    return items.filter((item) => item.file_name.toLowerCase().includes(needle));
  }, [items, query]);

  const run = async (file: string, action: () => Promise<unknown>) => {
    setBusy(file);
    setError(null);
    try {
      await action();
      await load();
    } catch (cause) {
      setError(String(cause));
    } finally {
      setBusy(null);
    }
  };

  const upload = async () => {
    const picked = await openFileDialog({
      multiple: true,
      filters: [{ name: "Jar files", extensions: ["jar"] }],
    });
    const sources = Array.isArray(picked) ? picked : picked ? [picked] : [];
    if (sources.length === 0) return;
    await run("", () => api.addServerContent(server.id, sources));
  };

  const disabled = items.filter((item) => !item.enabled).length;

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex flex-wrap items-center gap-2 px-8 py-3">
        <div className="relative w-full max-w-sm">
          <Search className="absolute left-3 top-1/2 size-3.5 -translate-y-1/2 text-content-faint" />
          <input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder={`Filter ${label.toLowerCase()}`}
            className="w-full rounded-lg border border-border bg-void py-1.5 pl-8 pr-3 text-[12px] text-content outline-none focus:border-(--accent)"
          />
        </div>

        <span className="font-pixel text-[10px] uppercase tracking-[0.28em] text-content-faint">
          {items.length} {label.toLowerCase()}
          {disabled > 0 && `, ${disabled} off`}
        </span>

        <div className="ml-auto flex items-center gap-2">
          {live && (
            <span className="font-pixel text-[10px] uppercase tracking-[0.22em] text-warn">
              restart to apply
            </span>
          )}
          <button
            onClick={() => void load()}
            title="Reload from disk"
            className="grid size-8 place-items-center rounded-lg border border-border bg-surface-2 text-content-muted transition-colors hover:bg-surface-3 hover:text-content"
          >
            <RotateCw className="size-3.5" />
          </button>
          <button
            onClick={() => void upload()}
            className="inline-flex items-center gap-2 rounded-lg border border-border bg-surface-2 px-3 py-1.5 text-[12px] font-medium text-content transition-colors hover:bg-surface-3"
          >
            <FileUp className="size-3.5" />
            Add files
          </button>
        </div>
      </div>

      {error && (
        <div className="wrap-break-word border-y border-danger/30 bg-danger/10 px-8 py-2 text-[11px] text-danger">
          {error}
        </div>
      )}

      <div className="min-h-0 flex-1 overflow-y-auto px-8 pb-8">
        {loading ? (
          <div className="py-10 text-center text-[12px] text-content-faint">Reading the folder</div>
        ) : shown.length === 0 ? (
          <div className="py-10 text-center text-[12px] text-content-faint">
            {items.length === 0 ? `Nothing here yet.` : "No match."}
          </div>
        ) : (
          shown.map((item) => (
            <div
              key={item.file_name}
              className="grid grid-cols-[minmax(0,1fr)_auto_auto_auto] items-center gap-4 border-b border-border-soft/50 py-2"
            >
              <span
                className={cn(
                  "truncate font-mono text-[12px]",
                  item.enabled ? "text-content" : "text-content-faint line-through",
                )}
                title={item.file_name}
              >
                {item.file_name}
              </span>
              <span className="text-[11px] tabular-nums text-content-faint">
                {formatBytes(item.size)}
              </span>
              <Toggle
                label={item.file_name}
                checked={item.enabled}
                onChange={() =>
                  void run(item.file_name, () =>
                    api.toggleServerContent(server.id, item.file_name),
                  )
                }
              />
              <button
                onClick={() => setDoomed(item.file_name)}
                disabled={busy === item.file_name}
                title="Delete"
                className="grid size-7 place-items-center rounded-md text-content-faint transition-colors hover:bg-danger/15 hover:text-danger disabled:opacity-40"
              >
                {busy === item.file_name ? (
                  <Loader2 className="size-3.5 animate-spin" />
                ) : (
                  <Trash2 className="size-3.5" />
                )}
              </button>
            </div>
          ))
        )}
      </div>

      <ConfirmDialog
        open={doomed !== null}
        title={`Delete ${doomed ?? ""}?`}
        description="The file is removed from the server folder. This cannot be undone."
        confirmLabel="Delete"
        onCancel={() => setDoomed(null)}
        onConfirm={() => {
          const file = doomed;
          setDoomed(null);
          if (file) void run(file, () => api.deleteServerContent(server.id, file));
        }}
      />
    </div>
  );
}
