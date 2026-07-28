import { useState } from "react";
import { Download, Heart, LayoutGrid, List, Package } from "lucide-react";

import { cn } from "../lib/cn";
import { relativeTime } from "../lib/time";
import type { ProjectSummary } from "../lib/types";

export type ResultView = "list" | "grid";

export function formatCount(count: number): string {
  if (count >= 1_000_000) return `${(count / 1_000_000).toFixed(1)}M`;
  if (count >= 1_000) return `${(count / 1_000).toFixed(0)}K`;
  return `${count}`;
}

export function accentFrom(color: number | null): string | undefined {
  if (color == null) return undefined;
  const r = (color >> 16) & 0xff;
  const g = (color >> 8) & 0xff;
  const b = color & 0xff;
  return `rgb(${r} ${g} ${b})`;
}

export function useResultView(storageKey: string): [ResultView, (v: ResultView) => void] {
  const [view, setView] = useState<ResultView>(
    () => (localStorage.getItem(storageKey) as ResultView) ?? "list",
  );
  const change = (v: ResultView) => {
    setView(v);
    localStorage.setItem(storageKey, v);
  };
  return [view, change];
}

export function ResultViewToggle({
  view,
  onChange,
}: {
  view: ResultView;
  onChange: (v: ResultView) => void;
}) {
  return (
    <div className="flex shrink-0 rounded-lg border border-border bg-surface-2 p-0.5">
      {(
        [
          { mode: "list", icon: List },
          { mode: "grid", icon: LayoutGrid },
        ] as const
      ).map(({ mode, icon: Icon }) => (
        <button
          key={mode}
          onClick={() => onChange(mode)}
          aria-label={`${mode} view`}
          className={cn(
            "grid size-8 place-items-center rounded-md transition-colors",
            view === mode
              ? "bg-surface-3 text-content"
              : "text-content-faint hover:text-content-muted",
          )}
        >
          <Icon className="size-4" />
        </button>
      ))}
    </div>
  );
}

function Icon({
  url,
  size,
  accent,
}: {
  url: string | null;
  size: string;
  accent?: string;
}) {
  return url ? (
    <img
      src={url}
      loading="lazy"
      style={accent ? { boxShadow: `0 0 0 1px ${accent}33` } : undefined}
      className={cn(size, "shrink-0 rounded-xl bg-surface-2 object-cover")}
      draggable={false}
    />
  ) : (
    <div
      className={cn(
        size,
        "grid shrink-0 place-items-center rounded-xl bg-surface-2 text-content-faint",
      )}
    >
      <Package className="size-5" />
    </div>
  );
}

function Tags({ items, max }: { items: string[]; max: number }) {
  const shown = items.slice(0, max);
  const more = items.length - shown.length;
  if (shown.length === 0) return null;
  return (
    <div className="flex flex-wrap items-center gap-1">
      {shown.map((tag) => (
        <span
          key={tag}
          className="rounded bg-surface-3 px-1.5 py-0.5 text-[10px] font-medium capitalize text-content-faint"
        >
          {tag}
        </span>
      ))}
      {more > 0 && <span className="text-[10px] text-content-faint">+{more}</span>}
    </div>
  );
}

function Stats({ project }: { project: ProjectSummary }) {
  return (
    <div className="flex shrink-0 items-center gap-2.5 text-[11px] text-content-faint">
      <span className="inline-flex items-center gap-1">
        <Download className="size-3" />
        {formatCount(project.downloads)}
      </span>
      {project.follows > 0 && (
        <span className="inline-flex items-center gap-1">
          <Heart className="size-3" />
          {formatCount(project.follows)}
        </span>
      )}
    </div>
  );
}

export interface ResultRow {
  project: ProjectSummary;
  subline?: string;
  onOpen: () => void;
  action: React.ReactNode;
}

export function ContentResults({ view, rows }: { view: ResultView; rows: ResultRow[] }) {
  if (view === "grid") {
    return (
      <div className="grid auto-rows-min grid-cols-[repeat(auto-fill,minmax(250px,1fr))] content-start gap-3">
        {rows.map(({ project, subline, onOpen, action }) => {
          const accent = accentFrom(project.color);
          return (
            <div
              key={project.id}
              onClick={onOpen}
              className="group relative flex cursor-pointer flex-col overflow-hidden rounded-xl border border-border-soft bg-surface-2/60 p-4 transition-colors hover:border-content-faint/30 hover:bg-surface-2"
            >
              {accent && (
                <span
                  className="absolute inset-x-0 top-0 h-0.5 opacity-0 transition-opacity group-hover:opacity-100"
                  style={{ background: accent }}
                />
              )}
              <div className="flex items-start gap-3">
                <Icon url={project.icon_url} size="size-12" accent={accent} />
                <div className="min-w-0 flex-1">
                  <div className="truncate text-sm font-semibold text-content">
                    {project.title}
                  </div>
                  <div className="truncate text-[11px] text-content-faint">
                    by {project.author}
                  </div>
                  <div className="mt-1">
                    <Stats project={project} />
                  </div>
                </div>
              </div>
              <p className="mt-2.5 line-clamp-2 min-h-[2.2rem] text-xs text-content-muted">
                {project.description}
              </p>
              <div className="mt-2">
                <Tags items={project.categories} max={3} />
              </div>
              {subline && (
                <div className="mt-1.5 truncate text-[11px] text-ok">{subline}</div>
              )}
              <div className="mt-auto flex items-center justify-between gap-2 pt-3">
                <span className="truncate text-[10px] text-content-faint">
                  {project.updated &&
                    `Updated ${relativeTime(
                      Math.floor(new Date(project.updated).getTime() / 1000),
                    )}`}
                </span>
                {action}
              </div>
            </div>
          );
        })}
      </div>
    );
  }

  return (
    <div className="flex flex-col">
      {rows.map(({ project, subline, onOpen, action }) => (
        <div
          key={project.id}
          onClick={onOpen}
          className="group flex cursor-pointer items-center gap-3 rounded-xl px-3 py-3 transition-colors hover:bg-surface-2"
        >
          <Icon url={project.icon_url} size="size-14" accent={accentFrom(project.color)} />
          <div className="min-w-0 flex-1">
            <div className="flex items-baseline gap-2">
              <span className="truncate text-sm font-semibold text-content">
                {project.title}
              </span>
              <span className="shrink-0 text-[11px] text-content-faint">
                by {project.author}
              </span>
            </div>
            <div className="truncate text-xs text-content-muted">{project.description}</div>
            <div className="mt-1 flex items-center gap-2.5">
              <Stats project={project} />
              <Tags items={project.categories} max={4} />
              {project.updated && (
                <span className="shrink-0 text-[10px] text-content-faint">
                  Updated{" "}
                  {relativeTime(Math.floor(new Date(project.updated).getTime() / 1000))}
                </span>
              )}
            </div>
            {subline && <div className="mt-0.5 truncate text-[11px] text-ok">{subline}</div>}
          </div>
          {action}
        </div>
      ))}
    </div>
  );
}
