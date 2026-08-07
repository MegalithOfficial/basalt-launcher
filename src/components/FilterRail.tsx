import { useMemo, useState } from "react";
import { ChevronDown, RotateCcw, Search } from "lucide-react";

import { cn } from "../lib/cn";
import type { Environment, FilterOption, FilterTaxonomy } from "../lib/types";

export interface FilterState {
  gameVersions: string[];
  loaders: string[];
  categories: string[];
  environment: Environment | null;
  openSourceOnly: boolean;
}

export const emptyFilters: FilterState = {
  gameVersions: [],
  loaders: [],
  categories: [],
  environment: null,
  openSourceOnly: false,
};

export function countActive(filters: FilterState): number {
  return (
    filters.gameVersions.length +
    filters.loaders.length +
    filters.categories.length +
    (filters.environment ? 1 : 0) +
    (filters.openSourceOnly ? 1 : 0)
  );
}

function toggle(list: string[], value: string): string[] {
  return list.includes(value) ? list.filter((v) => v !== value) : [...list, value];
}

function Section({
  title,
  children,
  defaultOpen = true,
  count,
}: {
  title: string;
  children: React.ReactNode;
  defaultOpen?: boolean;
  count?: number;
}) {
  const [open, setOpen] = useState(defaultOpen);
  return (
    <div className="border-b border-border-soft/60 pb-3 last:border-0">
      <button
        onClick={() => setOpen((v) => !v)}
        className="flex w-full items-center justify-between py-2 text-[11px] font-semibold uppercase tracking-wider text-content-faint transition-colors hover:text-content-muted"
      >
        <span className="flex items-center gap-1.5">
          {title}
          {!!count && (
            <span className="rounded-full bg-(--accent) px-1.5 text-[10px] font-bold text-black">
              {count}
            </span>
          )}
        </span>
        <ChevronDown className={cn("size-3.5 transition-transform", !open && "-rotate-90")} />
      </button>
      {open && <div className="mt-1 flex flex-col gap-0.5">{children}</div>}
    </div>
  );
}

function Check({
  label,
  checked,
  onClick,
}: {
  label: string;
  checked: boolean;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "flex items-center gap-2 rounded-lg px-2 py-1.5 text-left text-xs transition-colors",
        checked
          ? "bg-(--accent-glow) font-medium text-content"
          : "text-content-muted hover:bg-surface-2 hover:text-content",
      )}
    >
      <span
        className={cn(
          "grid size-3.5 shrink-0 place-items-center rounded border transition-colors",
          checked ? "border-(--accent) bg-(--accent)" : "border-border",
        )}
      >
        {checked && (
          <svg viewBox="0 0 10 10" className="size-2.5 text-black">
            <path
              d="M2 5.2l2 2 4-4.4"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.8"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </svg>
        )}
      </span>
      <span className="truncate capitalize">{label}</span>
    </button>
  );
}

const VERSIONS_COLLAPSED = 8;

export function FilterRail({
  taxonomy,
  filters,
  onChange,
  showLoaders,
  showEnvironment,
}: {
  taxonomy: FilterTaxonomy | null;
  filters: FilterState;
  onChange: (next: FilterState) => void;
  showLoaders: boolean;
  showEnvironment: boolean;
}) {
  const [versionQuery, setVersionQuery] = useState("");
  const [showAllVersions, setShowAllVersions] = useState(false);

  const grouped = useMemo(() => {
    const map = new Map<string, FilterOption[]>();
    for (const option of taxonomy?.categories ?? []) {
      const list = map.get(option.group) ?? [];
      list.push(option);
      map.set(option.group, list);
    }
    return [...map.entries()];
  }, [taxonomy]);

  const versions = useMemo(() => {
    const all = taxonomy?.game_versions ?? [];
    const query = versionQuery.trim().toLowerCase();
    const matched = query ? all.filter((v) => v.toLowerCase().includes(query)) : all;
    const selected = all.filter((v) => filters.gameVersions.includes(v));
    if (query || showAllVersions) return matched;
    const head = matched.slice(0, VERSIONS_COLLAPSED);
    return [...new Set([...selected, ...head])];
  }, [taxonomy, versionQuery, showAllVersions, filters.gameVersions]);

  const active = countActive(filters);

  return (
    <aside className="flex w-56 shrink-0 flex-col overflow-y-auto border-r border-border-soft px-4 pb-6">
      <div className="sticky top-0 z-10 flex items-center justify-between bg-void py-3">
        <span className="text-xs font-semibold text-content">Filters</span>
        {active > 0 && (
          <button
            onClick={() => onChange(emptyFilters)}
            className="inline-flex items-center gap-1 rounded-md px-1.5 py-1 text-[11px] text-content-faint transition-colors hover:bg-surface-2 hover:text-content"
          >
            <RotateCcw className="size-3" />
            Reset
          </button>
        )}
      </div>

      {showLoaders && (taxonomy?.loaders.length ?? 0) > 0 && (
        <Section title="Loaders" count={filters.loaders.length}>
          {taxonomy?.loaders.map((loader) => (
            <Check
              key={loader.id}
              label={loader.name}
              checked={filters.loaders.includes(loader.id)}
              onClick={() =>
                onChange({ ...filters, loaders: toggle(filters.loaders, loader.id) })
              }
            />
          ))}
        </Section>
      )}

      {(taxonomy?.game_versions.length ?? 0) > 0 && (
        <Section title="Game versions" count={filters.gameVersions.length}>
          <div className="relative mb-1">
            <Search className="absolute left-2 top-1/2 size-3 -translate-y-1/2 text-content-faint" />
            <input
              value={versionQuery}
              onChange={(e) => setVersionQuery(e.target.value)}
              placeholder="Filter"
              className="w-full rounded-md border border-border bg-void py-1 pl-6 pr-2 text-[11px] text-content outline-none focus:border-(--accent)"
            />
          </div>
          {versions.map((version) => (
            <Check
              key={version}
              label={version}
              checked={filters.gameVersions.includes(version)}
              onClick={() =>
                onChange({ ...filters, gameVersions: toggle(filters.gameVersions, version) })
              }
            />
          ))}
          {!versionQuery && (taxonomy?.game_versions.length ?? 0) > VERSIONS_COLLAPSED && (
            <button
              onClick={() => setShowAllVersions((v) => !v)}
              className="mt-0.5 px-2 py-1 text-left text-[11px] font-medium text-(--accent) hover:underline"
            >
              {showAllVersions
                ? "Show fewer"
                : `Show all ${taxonomy?.game_versions.length}`}
            </button>
          )}
        </Section>
      )}

      {showEnvironment && (
        <Section title="Environment" count={filters.environment ? 1 : 0} defaultOpen={false}>
          {(["client", "server"] as const).map((env) => (
            <Check
              key={env}
              label={env}
              checked={filters.environment === env}
              onClick={() =>
                onChange({
                  ...filters,
                  environment: filters.environment === env ? null : env,
                })
              }
            />
          ))}
          <Check
            label="Open source"
            checked={filters.openSourceOnly}
            onClick={() => onChange({ ...filters, openSourceOnly: !filters.openSourceOnly })}
          />
        </Section>
      )}

      {grouped.map(([group, options]) => (
        <Section
          key={group}
          title={group}
          defaultOpen={group.toLowerCase() === "categories"}
          count={options.filter((o) => filters.categories.includes(o.id)).length}
        >
          {options.map((option) => (
            <Check
              key={option.id}
              label={option.name}
              checked={filters.categories.includes(option.id)}
              onClick={() =>
                onChange({ ...filters, categories: toggle(filters.categories, option.id) })
              }
            />
          ))}
        </Section>
      ))}
    </aside>
  );
}
