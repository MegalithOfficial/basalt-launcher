import { useState } from "react";
import { LayoutGrid, List, Package } from "lucide-react";

import { cn } from "../lib/cn";

export type ResultView = "list" | "grid";

export interface ResultItem {
  key: string;
  iconUrl: string | null;
  title: string;
  meta: string;
  description: string;
  subline?: string;
  onOpen: () => void;
  action: React.ReactNode;
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

function Icon({ url, size }: { url: string | null; size: string }) {
  return url ? (
    <img src={url} className={cn(size, "shrink-0 rounded-xl bg-surface-2 object-cover")} draggable={false} />
  ) : (
    <div className={cn(size, "grid shrink-0 place-items-center rounded-xl bg-surface-2 text-content-faint")}>
      <Package className="size-5" />
    </div>
  );
}

export function ContentResults({ view, items }: { view: ResultView; items: ResultItem[] }) {
  if (view === "grid") {
    return (
      <div className="grid auto-rows-min grid-cols-[repeat(auto-fill,minmax(240px,1fr))] content-start gap-3">
        {items.map((item) => (
          <div
            key={item.key}
            onClick={item.onOpen}
            className="group flex cursor-pointer flex-col rounded-xl border border-border-soft bg-surface-2/60 p-4 transition-colors hover:border-content-faint/30 hover:bg-surface-2"
          >
            <div className="flex items-start gap-3">
              <Icon url={item.iconUrl} size="size-12" />
              <div className="min-w-0 flex-1">
                <div className="truncate text-sm font-semibold text-content">{item.title}</div>
                <div className="truncate text-[11px] text-content-faint">{item.meta}</div>
              </div>
            </div>
            <p className="mt-2.5 line-clamp-2 min-h-[2.2rem] text-xs text-content-muted">
              {item.description}
            </p>
            {item.subline && (
              <div className="mb-2 truncate text-[11px] text-ok">{item.subline}</div>
            )}
            <div className="mt-auto flex justify-end pt-2">{item.action}</div>
          </div>
        ))}
      </div>
    );
  }

  return (
    <div className="flex flex-col">
      {items.map((item) => (
        <div
          key={item.key}
          onClick={item.onOpen}
          className="group flex cursor-pointer items-center gap-3 rounded-xl px-3 py-3 transition-colors hover:bg-surface-2"
        >
          <Icon url={item.iconUrl} size="size-12" />
          <div className="min-w-0 flex-1">
            <div className="flex items-baseline gap-2">
              <span className="truncate text-sm font-semibold text-content">{item.title}</span>
              <span className="shrink-0 text-[11px] text-content-faint">{item.meta}</span>
            </div>
            <div className="truncate text-xs text-content-muted">{item.description}</div>
            {item.subline && (
              <div className="mt-0.5 truncate text-[11px] text-ok">{item.subline}</div>
            )}
          </div>
          {item.action}
        </div>
      ))}
    </div>
  );
}
