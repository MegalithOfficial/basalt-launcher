import { useMemo, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  Boxes,
  Check,
  ChevronDown,
  Download,
  Server as ServerIcon,
  ExternalLink,
  Loader2,
  Package,
} from "lucide-react";

import { cn } from "../../lib/cn";
import { serverPackFile } from "../../lib/servers";
import { formatDateTime, relativeTime } from "../../lib/time";
import type {
  Changelog,
  ContentKind,
  ProjectSummary,
  ProjectVersion,
} from "../../lib/types";
import { formatCount } from "../ContentResults";
import { Select } from "../Select";
import { Markdown } from "./Markdown";
import { formatBytes } from "../../lib/format";

const PAGE_SIZE = 50;

type Channel = "all" | "release" | "beta" | "alpha";

const CHANNEL_STYLE: Record<string, string> = {
  release: "bg-ok/15 text-ok",
  beta: "bg-warn/15 text-warn",
  alpha: "bg-danger/15 text-danger",
};

const DEP_STYLE: Record<string, string> = {
  required: "bg-danger/15 text-danger",
  optional: "bg-surface-3 text-content-muted",
  embedded: "bg-surface-3 text-content-faint",
  incompatible: "bg-warn/15 text-warn",
};


function FilterGroup({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div>
      <div className="mb-1.5 text-[10px] font-semibold uppercase tracking-wider text-content-faint">
        {label}
      </div>
      {children}
    </div>
  );
}

function Segmented<T extends string>({
  options,
  value,
  onChange,
}: {
  options: Array<{ id: T; label: string }>;
  value: T;
  onChange: (v: T) => void;
}) {
  return (
    <div className="flex rounded-lg border border-border bg-surface-2 p-0.5">
      {options.map((option) => (
        <button
          key={option.id}
          onClick={() => onChange(option.id)}
          className={cn(
            "rounded-md px-3 py-1.5 text-xs font-medium capitalize transition-colors",
            value === option.id
              ? "bg-surface-3 text-content"
              : "text-content-faint hover:text-content-muted",
          )}
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}

export interface VersionBrowserProps {
  versions: ProjectVersion[];
  kind: ContentKind;
  isPack: boolean;
  instanceVersion: string | null;
  instanceLoader: string | null;
  hasInstance: boolean;
  installedVersionId: string | null;
  installingKey: string | null;
  installedKeys: Set<string>;
  resolvedProjects: Record<string, ProjectSummary | null>;
  changelogs: Record<string, Changelog | "loading">;
  websiteUrl: string | null;
  provider: string;
  onExpand: (version: ProjectVersion) => void;
  expandedId: string | null;
  onInstall: (versionId: string) => void;
  onGetServer?: (version: ProjectVersion) => void;
  onInstallDependency: (project: ProjectSummary) => void;
  onOpenProject: (projectId: string) => void;
  onChooseInstance: () => void;
}

export function VersionBrowser({
  versions,
  kind,
  isPack,
  instanceVersion,
  instanceLoader,
  hasInstance,
  installedVersionId,
  installingKey,
  installedKeys,
  resolvedProjects,
  changelogs,
  websiteUrl,
  provider,
  onExpand,
  expandedId,
  onInstall,
  onGetServer,
  onInstallDependency,
  onOpenProject,
  onChooseInstance,
}: VersionBrowserProps) {
  const [channel, setChannel] = useState<Channel>("all");
  const [compatibleOnly, setCompatibleOnly] = useState(hasInstance && !isPack);
  const [gvFilter, setGvFilter] = useState("all");
  const [loaderFilter, setLoaderFilter] = useState("all");
  const [sortBy, setSortBy] = useState<"newest" | "downloads">("newest");
  const [visible, setVisible] = useState(PAGE_SIZE);

  const { gvOptions, loaderOptions } = useMemo(() => {
    const gv: string[] = [];
    const ld: string[] = [];
    for (const v of versions) {
      for (const g of v.game_versions) if (!gv.includes(g)) gv.push(g);
      for (const l of v.loaders) if (!ld.includes(l)) ld.push(l);
    }
    return { gvOptions: gv, loaderOptions: ld };
  }, [versions]);

  const sorted = useMemo(() => {
    const filtered = versions.filter(
      (v) =>
        (channel === "all" || v.channel === channel) &&
        (!compatibleOnly || v.compatible) &&
        (gvFilter === "all" || v.game_versions.includes(gvFilter)) &&
        (loaderFilter === "all" || v.loaders.includes(loaderFilter)),
    );
    return sortBy === "downloads"
      ? [...filtered].sort((a, b) => b.downloads - a.downloads)
      : filtered;
  }, [versions, channel, compatibleOnly, gvFilter, loaderFilter, sortBy]);

  const shown = sorted.slice(0, visible);
  const compatibleCount = versions.filter((v) => v.compatible).length;
  const installedRow = versions.find((v) => v.id === installedVersionId);
  const installedDate = installedRow ? new Date(installedRow.date).getTime() : null;

  return (
    <>
      <div className="mb-4 flex flex-wrap items-end gap-x-5 gap-y-3">
        <FilterGroup label="Game version">
          <div className="w-48">
            <Select
              value={gvFilter === "all" ? "All game versions" : gvFilter}
              options={["All game versions", ...gvOptions]}
              onChange={(v) => setGvFilter(v === "All game versions" ? "all" : v)}
            />
          </div>
        </FilterGroup>

        {kind === "mods" && loaderOptions.length > 1 && (
          <FilterGroup label="Loader">
            <div className="w-40">
              <Select
                value={loaderFilter === "all" ? "All loaders" : loaderFilter}
                options={["All loaders", ...loaderOptions]}
                onChange={(v) => setLoaderFilter(v === "All loaders" ? "all" : v)}
              />
            </div>
          </FilterGroup>
        )}

        <FilterGroup label="Channel">
          <Segmented
            value={channel}
            onChange={setChannel}
            options={[
              { id: "all", label: "All" },
              { id: "release", label: "Release" },
              { id: "beta", label: "Beta" },
              { id: "alpha", label: "Alpha" },
            ]}
          />
        </FilterGroup>

        <FilterGroup label="Sort">
          <Segmented
            value={sortBy}
            onChange={setSortBy}
            options={[
              { id: "newest", label: "Newest" },
              { id: "downloads", label: "Popular" },
            ]}
          />
        </FilterGroup>

        {!isPack &&
          (hasInstance ? (
            <FilterGroup label="Compatibility">
              <button
                onClick={() => setCompatibleOnly((v) => !v)}
                className={cn(
                  "inline-flex items-center gap-1.5 rounded-lg border px-3 py-1.75 text-xs font-medium transition-colors",
                  compatibleOnly
                    ? "border-ok/40 bg-ok/10 text-ok"
                    : "border-border bg-surface-2 text-content-muted hover:text-content",
                )}
              >
                <Check className={cn("size-3.5", !compatibleOnly && "opacity-30")} />
                {instanceVersion}
                {instanceLoader && ` · ${instanceLoader}`}
              </button>
            </FilterGroup>
          ) : (
            <FilterGroup label="Compatibility">
              <button
                onClick={onChooseInstance}
                className="inline-flex items-center gap-1.5 rounded-lg border border-dashed border-border px-3 py-1.75 text-xs font-medium text-content-faint transition-colors hover:text-content"
              >
                <Boxes className="size-3.5" />
                Pick an instance to check
              </button>
            </FilterGroup>
          ))}

        <div className="ml-auto self-end pb-1.5 text-xs text-content-faint">
          {shown.length} of {sorted.length} shown
          {hasInstance && !isPack && ` · ${compatibleCount} compatible`} · {versions.length} total
        </div>
      </div>

      {shown.length === 0 ? (
        <div className="py-12 text-center text-sm text-content-faint">
          Nothing matches these filters.
        </div>
      ) : (
        <div className="flex flex-col gap-1.5">
          {shown.map((v) => {
            const done = installedKeys.has(v.id);
            const busy = installingKey === v.id;
            const expanded = expandedId === v.id;
            const changelog = changelogs[v.id];
            const isInstalledRow = installedVersionId === v.id;
            const usable = isPack || !hasInstance || v.compatible;
            const isUpdate =
              !isPack &&
              !isInstalledRow &&
              installedDate !== null &&
              v.compatible &&
              new Date(v.date).getTime() > installedDate;

            return (
              <div
                key={v.id}
                className={cn(
                  "rounded-xl border transition-colors",
                  isInstalledRow
                    ? "border-(--accent) bg-(--accent-glow)"
                    : hasInstance && !isPack && v.compatible
                      ? "border-ok/35 bg-ok/5"
                      : "border-border-soft bg-surface-2/40",
                )}
              >
                <div
                  className={cn(
                    "grid cursor-pointer grid-cols-[4.5rem_minmax(0,1fr)_auto_auto_auto] items-center gap-3 px-4 py-2.5",
                    !usable && !isInstalledRow && "opacity-60",
                  )}
                  onClick={() => onExpand(v)}
                >
                  <span
                    className={cn(
                      "rounded px-1.5 py-0.5 text-center text-[10px] font-semibold uppercase tracking-wide",
                      CHANNEL_STYLE[v.channel] ?? CHANNEL_STYLE.release,
                    )}
                  >
                    {v.channel}
                  </span>

                  <div className="min-w-0">
                    <div className="flex items-center gap-2">
                      <span className="truncate text-sm font-medium text-content">{v.name}</span>
                      {isInstalledRow && (
                        <span className="shrink-0 rounded bg-(--accent) px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-wide text-black">
                          Installed
                        </span>
                      )}
                      {v.dependencies.length > 0 && (
                        <span className="shrink-0 text-[10px] text-content-faint">
                          {v.dependencies.length} dep{v.dependencies.length > 1 ? "s" : ""}
                        </span>
                      )}
                    </div>
                    <div className="mt-0.5 flex items-center gap-1.5">
                      {v.game_versions.slice(0, 3).map((g) => (
                        <span
                          key={g}
                          className={cn(
                            "rounded px-1.5 py-0.5 text-[10px] font-medium",
                            g === instanceVersion
                              ? "bg-ok/20 text-ok"
                              : "bg-surface-3 text-content-faint",
                          )}
                        >
                          {g}
                        </span>
                      ))}
                      {v.game_versions.length > 3 && (
                        <span className="text-[10px] text-content-faint">
                          +{v.game_versions.length - 3}
                        </span>
                      )}
                      {v.loaders.slice(0, 3).map((l) => (
                        <span
                          key={l}
                          className="rounded bg-(--accent-glow) px-1.5 py-0.5 text-[10px] font-medium capitalize text-content-muted"
                        >
                          {l}
                        </span>
                      ))}
                    </div>
                  </div>

                  <div className="hidden text-right text-[11px] leading-tight text-content-faint sm:block">
                    <div>{formatCount(v.downloads)} downloads</div>
                    <div>
                      {v.size != null && `${formatBytes(v.size)} · `}
                      {v.date && relativeTime(Math.floor(new Date(v.date).getTime() / 1000))}
                    </div>
                  </div>

                  <div className="flex shrink-0 items-center gap-1.5">
                  {isInstalledRow ? (
                    <span className="inline-flex h-8 shrink-0 items-center gap-1.5 rounded-lg bg-ok/15 px-3 text-xs font-semibold text-ok">
                      <Check className="size-3.5" />
                      Current
                    </span>
                  ) : (
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        onInstall(v.id);
                      }}
                      disabled={busy || done || installingKey !== null || !usable}
                      title={usable ? undefined : "Not compatible with this instance"}
                      className={cn(
                        "inline-flex h-8 shrink-0 items-center gap-1.5 rounded-lg px-3 text-xs font-semibold transition-all",
                        done
                          ? "cursor-default bg-ok/15 text-ok"
                          : isUpdate
                            ? "bg-warn/15 text-warn hover:bg-warn/25 disabled:opacity-50"
                            : usable
                              ? "bg-ok/15 text-ok hover:bg-ok/25 disabled:opacity-50"
                              : "border border-border bg-surface-3 text-content-faint",
                      )}
                    >
                      {done ? (
                        <>
                          <Check className="size-3.5" />
                          Added
                        </>
                      ) : busy ? (
                        <Loader2 className="size-3.5 animate-spin" />
                      ) : (
                        <>
                          <Download className="size-3.5" />
                          {isUpdate ? "Update" : "Install"}
                        </>
                      )}
                    </button>
                  )}

                  {onGetServer && (v.server_pack_file_id || serverPackFile(v.files)) && (
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        onGetServer(v);
                      }}
                      aria-label="Get the server pack"
                      title="Install the server pack this version ships"
                      className="grid size-8 shrink-0 place-items-center rounded-lg text-content-faint transition-colors hover:bg-surface-3 hover:text-content"
                    >
                      <ServerIcon className="size-4" />
                    </button>
                  )}
                  </div>

                  <ChevronDown
                    className={cn(
                      "size-4 shrink-0 text-content-faint transition-transform",
                      expanded && "rotate-180",
                    )}
                  />
                </div>

                {expanded && (
                  <div className="border-t border-border-soft px-4 py-3">
                    <div className="mb-3">
                      <div className="mb-1.5 text-[11px] font-semibold uppercase tracking-wide text-content-faint">
                        Supports
                      </div>
                      <div className="flex flex-wrap gap-1.5">
                        {v.game_versions.map((g) => (
                          <span
                            key={g}
                            className={cn(
                              "rounded px-1.5 py-0.5 text-[10px] font-medium",
                              g === instanceVersion
                                ? "bg-ok/20 text-ok"
                                : "bg-surface-3 text-content-muted",
                            )}
                          >
                            {g}
                          </span>
                        ))}
                        {v.loaders.map((l) => (
                          <span
                            key={l}
                            className="rounded bg-(--accent-glow) px-1.5 py-0.5 text-[10px] font-medium capitalize text-content-muted"
                          >
                            {l}
                          </span>
                        ))}
                      </div>
                    </div>

                    {v.files.filter((file) => !file.primary).length > 0 && (
                      <div className="mb-3">
                        <div className="mb-1.5 text-[11px] font-semibold uppercase tracking-wide text-content-faint">
                          Additional files
                        </div>
                        <div className="flex flex-col gap-1">
                          {v.files
                            .filter((file) => !file.primary)
                            .map((file) => {
                              const isServer = serverPackFile([file]) !== undefined;
                              return (
                                <div
                                  key={file.file_name}
                                  className="flex items-center gap-2 rounded-lg border border-border-soft bg-surface-2/60 px-2.5 py-1.5"
                                >
                                  <span className="min-w-0 flex-1 truncate font-mono text-[11px] text-content-muted">
                                    {file.file_name}
                                  </span>
                                  {isServer && (
                                    <span className="shrink-0 rounded bg-(--accent-glow) px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-wide text-content-muted">
                                      server pack
                                    </span>
                                  )}
                                  <span className="shrink-0 text-[10px] tabular-nums text-content-faint">
                                    {formatBytes(file.size ?? 0)}
                                  </span>
                                  {file.url && (
                                    <button
                                      onClick={() => void openUrl(file.url!)}
                                      title="Download in your browser"
                                      className="grid size-6 shrink-0 place-items-center rounded text-content-faint transition-colors hover:bg-surface-3 hover:text-content"
                                    >
                                      <Download className="size-3.5" />
                                    </button>
                                  )}
                                </div>
                              );
                            })}
                        </div>
                      </div>
                    )}

                    {v.dependencies.length > 0 && (
                      <div className="mb-3">
                        <div className="mb-1.5 text-[11px] font-semibold uppercase tracking-wide text-content-faint">
                          Dependencies
                        </div>
                        <div className="flex flex-col gap-1">
                          {v.dependencies.map((dep) => {
                            const info = resolvedProjects[dep.project_id];
                            const depKey = `dep:${dep.project_id}`;
                            const depBusy = installingKey === depKey;
                            const depDone = installedKeys.has(depKey);
                            return (
                              <div
                                key={dep.project_id}
                                className="flex items-center gap-2.5 rounded-lg bg-surface-2/60 px-2.5 py-1.5"
                              >
                                {info?.icon_url ? (
                                  <img
                                    src={info.icon_url}
                                    className="size-6 shrink-0 rounded-md bg-surface-3 object-cover"
                                    draggable={false}
                                  />
                                ) : (
                                  <div className="grid size-6 shrink-0 place-items-center rounded-md bg-surface-3 text-content-faint">
                                    <Package className="size-3" />
                                  </div>
                                )}
                                <button
                                  onClick={() => onOpenProject(dep.project_id)}
                                  className="min-w-0 truncate text-left text-xs font-medium text-content hover:underline"
                                >
                                  {info?.title ?? dep.project_id}
                                </button>
                                <span
                                  className={cn(
                                    "shrink-0 rounded px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-wide",
                                    DEP_STYLE[dep.dependency_type] ?? DEP_STYLE.optional,
                                  )}
                                >
                                  {dep.dependency_type}
                                </span>
                                {(dep.dependency_type === "required" ||
                                  dep.dependency_type === "optional") &&
                                  info && (
                                    <button
                                      onClick={() => onInstallDependency(info)}
                                      disabled={depBusy || depDone}
                                      className={cn(
                                        "ml-auto inline-flex h-6 shrink-0 items-center gap-1 rounded-md px-2 text-[10px] font-semibold",
                                        depDone
                                          ? "bg-ok/15 text-ok"
                                          : "bg-surface-3 text-content hover:bg-border",
                                      )}
                                    >
                                      {depDone ? (
                                        <Check className="size-3" />
                                      ) : depBusy ? (
                                        <Loader2 className="size-3 animate-spin" />
                                      ) : (
                                        "Install"
                                      )}
                                    </button>
                                  )}
                              </div>
                            );
                          })}
                        </div>
                      </div>
                    )}

                    {changelog === "loading" || !changelog ? (
                      <div className="flex items-center gap-2 py-2 text-xs text-content-muted">
                        <Loader2 className="size-3.5 animate-spin" />
                        Loading changelog
                      </div>
                    ) : changelog.body.trim() ? (
                      <Markdown
                        body={changelog.body}
                        format={changelog.format}
                        className="text-xs"
                      />
                    ) : (
                      <div className="text-xs text-content-faint">No changelog provided.</div>
                    )}

                    <div className="mt-3 flex items-center gap-3 border-t border-border-soft pt-2 text-[10px] text-content-faint">
                      <span>{v.version_number}</span>
                      {v.date && <span>{formatDateTime(v.date)}</span>}
                      {websiteUrl && (
                        <button
                          onClick={() =>
                            openUrl(
                              provider === "modrinth"
                                ? `${websiteUrl}/version/${v.id}`
                                : `${websiteUrl}/files/${v.id}`,
                            )
                          }
                          className="ml-auto inline-flex items-center gap-1 text-content-muted hover:text-content"
                        >
                          Open in browser
                          <ExternalLink className="size-3" />
                        </button>
                      )}
                    </div>
                  </div>
                )}
              </div>
            );
          })}

          {visible < sorted.length && (
            <button
              onClick={() => setVisible((n) => n + PAGE_SIZE)}
              className="mt-1 rounded-xl border border-border bg-surface-2 py-2.5 text-xs font-medium text-content-muted transition-colors hover:bg-surface-3 hover:text-content"
            >
              Load more ({sorted.length - visible} remaining)
            </button>
          )}
        </div>
      )}
    </>
  );
}
