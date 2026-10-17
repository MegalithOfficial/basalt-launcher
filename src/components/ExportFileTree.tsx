import { useCallback, useEffect, useState } from "react";
import { Check, ChevronRight, File, Folder, Loader2, Minus } from "lucide-react";

import { api } from "../lib/api";
import { cn } from "../lib/cn";
import { formatBytes } from "../lib/format";
import type { ExportCandidate, PackFormat } from "../lib/types";

export interface ExportRules {
  included: string[];
  excluded: string[];
}

/**
 * Rules are paths with a yes or no. A path takes the nearest rule on its own
 * chain, nothing ruled means out. Same resolution as the Rust side.
 */
function ruleFor(rules: ExportRules, path: string): boolean | null {
  if (rules.included.includes(path)) return true;
  if (rules.excluded.includes(path)) return false;
  return null;
}

export function resolveSelected(rules: ExportRules, path: string): boolean {
  const parts = path.split("/");
  for (let depth = parts.length; depth > 0; depth--) {
    const rule = ruleFor(rules, parts.slice(0, depth).join("/"));
    if (rule !== null) return rule;
  }
  return false;
}

function hasRulesBelow(rules: ExportRules, path: string, value: boolean): boolean {
  const list = value ? rules.included : rules.excluded;
  return list.some((rule) => rule.startsWith(`${path}/`));
}

function withoutBelow(list: string[], path: string) {
  return list.filter((rule) => rule !== path && !rule.startsWith(`${path}/`));
}

export function toggle(rules: ExportRules, path: string, selected: boolean): ExportRules {
  const included = withoutBelow(rules.included, path);
  const excluded = withoutBelow(rules.excluded, path);
  const inherited = resolveSelected({ included, excluded }, path);
  if (inherited === selected) return { included, excluded };
  return selected
    ? { included: [...included, path], excluded }
    : { included, excluded: [...excluded, path] };
}

function linkedCount(item: ExportCandidate, format: PackFormat) {
  return format === "mrpack" ? item.modrinth : item.curseforge;
}

function Row({
  item,
  depth,
  rules,
  format,
  onToggle,
  instanceId,
}: {
  item: ExportCandidate;
  depth: number;
  rules: ExportRules;
  format: PackFormat;
  onToggle: (path: string, selected: boolean) => void;
  instanceId: string;
}) {
  const [open, setOpen] = useState(false);
  const [children, setChildren] = useState<ExportCandidate[] | null>(null);
  const [loading, setLoading] = useState(false);
  const selected = resolveSelected(rules, item.path);
  const mixed = item.directory && hasRulesBelow(rules, item.path, !selected);
  const linked = linkedCount(item, format);
  const lit = selected || mixed;

  const expand = useCallback(async () => {
    if (!item.directory) return;
    setOpen((value) => !value);
    if (children !== null) return;
    setLoading(true);
    try {
      setChildren(await api.listExportCandidates(instanceId, item.path));
    } catch {
      setChildren([]);
    } finally {
      setLoading(false);
    }
  }, [children, instanceId, item.directory, item.path]);

  const name = item.path.slice(item.path.lastIndexOf("/") + 1);
  const Glyph = item.directory ? Folder : File;

  return (
    <>
      <div
        role="checkbox"
        aria-checked={mixed ? "mixed" : selected}
        tabIndex={0}
        onClick={() => onToggle(item.path, !selected)}
        onKeyDown={(event) => {
          if (event.key === " " || event.key === "Enter") {
            event.preventDefault();
            onToggle(item.path, !selected);
          }
        }}
        className={cn(
          "flex cursor-pointer select-none items-center gap-2 rounded-lg py-1.5 pr-2.5 text-[13px] outline-none transition-colors hover:bg-surface-2 focus-visible:ring-1 focus-visible:ring-(--accent)",
          lit ? "text-content" : "text-content-faint",
        )}
        style={{ paddingLeft: 6 + depth * 18 }}
      >
        <button
          onClick={(event) => {
            event.stopPropagation();
            void expand();
          }}
          disabled={!item.directory}
          tabIndex={-1}
          className="grid size-5 shrink-0 place-items-center rounded text-content-faint transition-colors hover:text-content disabled:invisible"
          aria-label={open ? "Collapse" : "Expand"}
        >
          {loading ? (
            <Loader2 className="size-3 animate-spin" />
          ) : (
            <ChevronRight className={cn("size-3 transition-transform", open && "rotate-90")} />
          )}
        </button>
        <span
          className={cn(
            "grid size-[15px] shrink-0 place-items-center rounded-[4px] border transition-colors",
            selected
              ? "border-(--accent) bg-(--accent) text-void"
              : mixed
                ? "border-(--accent) bg-surface-2 text-(--accent)"
                : "border-border bg-surface-2",
          )}
        >
          {selected && <Check className="size-2.5" strokeWidth={3.5} />}
          {!selected && mixed && <Minus className="size-2.5" strokeWidth={3.5} />}
        </span>
        <Glyph
          className={cn("size-3.5 shrink-0", lit ? "text-content-muted" : "text-content-faint/60")}
        />
        <span className="min-w-0 flex-1 truncate">{name}</span>
        {linked > 0 ? (
          <span
            className={cn(
              "shrink-0 font-mono text-[10px]",
              lit ? "text-ok" : "text-content-faint",
            )}
          >
            {item.directory ? `${linked} by link` : "by link"}
          </span>
        ) : (
          !item.directory && (
            <span className="shrink-0 font-mono text-[10px] text-content-faint">
              {formatBytes(item.size)}
            </span>
          )
        )}
      </div>
      {open &&
        children?.map((child) => (
          <Row
            key={child.path}
            item={child}
            depth={depth + 1}
            rules={rules}
            format={format}
            onToggle={onToggle}
            instanceId={instanceId}
          />
        ))}
      {open && children?.length === 0 && !loading && (
        <div
          className="py-1 text-[11px] text-content-faint"
          style={{ paddingLeft: 6 + (depth + 1) * 18 + 28 }}
        >
          Empty
        </div>
      )}
    </>
  );
}

export function defaultRules(root: ExportCandidate[]): ExportRules {
  return {
    included: root.filter((item) => item.default_selected).map((item) => item.path),
    excluded: [],
  };
}

export function ExportFileTree({
  instanceId,
  format,
  rules,
  onRulesChange,
  onLoaded,
}: {
  instanceId: string;
  format: PackFormat;
  rules: ExportRules;
  onRulesChange: (rules: ExportRules) => void;
  onLoaded: (root: ExportCandidate[]) => void;
}) {
  const [root, setRoot] = useState<ExportCandidate[] | null>(null);

  useEffect(() => {
    let live = true;
    setRoot(null);
    api
      .listExportCandidates(instanceId, null)
      .then((items) => {
        if (!live) return;
        setRoot(items);
        onLoaded(items);
      })
      .catch(() => live && setRoot([]));
    return () => {
      live = false;
    };
  }, [instanceId, onLoaded]);

  const chosen = root?.filter((item) => resolveSelected(rules, item.path)) ?? [];
  const folders = chosen.filter((item) => item.directory).length;
  const files = chosen.length - folders;
  const summary = [
    folders > 0 && `${folders} ${folders === 1 ? "folder" : "folders"}`,
    files > 0 && `${files} ${files === 1 ? "file" : "files"}`,
  ]
    .filter(Boolean)
    .join(", ");

  return (
    <div className="flex min-h-0 flex-col overflow-hidden rounded-xl border border-border-soft bg-void/40">
      <div className="flex items-center gap-2.5 border-b border-border-soft px-3 py-2">
        <span className="font-pixel text-[9px] tracking-[0.14em] text-content-faint uppercase">
          What travels
        </span>
        <span className="text-[11px] text-content-muted">{summary || "Nothing yet"}</span>
        {root && (
          <button
            onClick={() => onRulesChange(defaultRules(root))}
            className="ml-auto text-[11px] text-content-faint transition-colors hover:text-content"
          >
            Reset to defaults
          </button>
        )}
      </div>
      {root === null ? (
        <div className="flex items-center justify-center py-10 text-content-faint">
          <Loader2 className="size-4 animate-spin" />
        </div>
      ) : (
        <div className="max-h-[42vh] min-h-48 overflow-y-auto p-1.5">
          {root.map((item) => (
            <Row
              key={item.path}
              item={item}
              depth={0}
              rules={rules}
              format={format}
              onToggle={(path, selected) => onRulesChange(toggle(rules, path, selected))}
              instanceId={instanceId}
            />
          ))}
          {root.length === 0 && (
            <div className="py-6 text-center text-xs text-content-faint">Nothing to export.</div>
          )}
        </div>
      )}
    </div>
  );
}
