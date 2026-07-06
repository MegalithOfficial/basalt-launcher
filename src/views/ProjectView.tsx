import { useEffect, useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeRaw from "rehype-raw";
import rehypeSanitize, { defaultSchema } from "rehype-sanitize";
import DOMPurify from "dompurify";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  ArrowLeft,
  Check,
  ChevronDown,
  Download,
  ExternalLink,
  Loader2,
  Package,
  TriangleAlert,
  User,
} from "lucide-react";

import { cn } from "../lib/cn";
import { api } from "../lib/api";
import { relativeTime } from "../lib/time";
import type {
  Changelog,
  InstalledFile,
  ProjectDetails,
  ProjectVersion,
  SearchResult,
} from "../lib/types";
import { Select } from "../components/Select";
import { formatDownloads } from "./SearchView";
import { useStore } from "../store";

const PAGE_SIZE = 50;

const DEP_STYLE: Record<string, string> = {
  required: "bg-danger/15 text-danger",
  optional: "bg-surface-3 text-content-muted",
  embedded: "bg-surface-3 text-content-faint",
  incompatible: "bg-warn/15 text-warn",
};

type Channel = "all" | "release" | "beta" | "alpha";

function ChangelogBody({ changelog }: { changelog: Changelog }) {
  if (!changelog.body.trim()) {
    return <div className="text-xs text-content-faint">No changelog provided.</div>;
  }
  if (changelog.format === "markdown") {
    return (
      <div className="prose-basalt text-xs">
        <ReactMarkdown remarkPlugins={[remarkGfm]}>{changelog.body}</ReactMarkdown>
      </div>
    );
  }
  return (
    <div
      className="prose-basalt text-xs"
      dangerouslySetInnerHTML={{ __html: DOMPurify.sanitize(changelog.body) }}
    />
  );
}

const sanitizeSchema = {
  ...defaultSchema,
  tagNames: [...(defaultSchema.tagNames ?? []), "center", "details", "summary"],
  attributes: {
    ...defaultSchema.attributes,
    img: [...(defaultSchema.attributes?.img ?? []), "width", "height", "align"],
    "*": [...(defaultSchema.attributes?.["*"] ?? []), "align"],
  },
};

type Tab = "description" | "versions" | "gallery";

const CHANNEL_STYLE: Record<string, string> = {
  release: "bg-ok/15 text-ok",
  beta: "bg-warn/15 text-warn",
  alpha: "bg-danger/15 text-danger",
};

function formatSize(bytes: number | null): string {
  if (bytes == null) return "";
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function environmentLabel(clientSide: string | null, serverSide: string | null): string | null {
  if (!clientSide && !serverSide) return null;
  if (clientSide === "required" && serverSide === "unsupported") return "Client-side";
  if (serverSide === "required" && clientSide === "unsupported") return "Server-side";
  return "Client and server";
}

function SideCard({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="rounded-2xl border border-border-soft bg-surface-2/60 p-4">
      <div className="mb-2.5 text-sm font-semibold text-content">{title}</div>
      {children}
    </div>
  );
}

function Chip({ children, tone = "default" }: { children: React.ReactNode; tone?: "default" | "accent" }) {
  return (
    <span
      className={cn(
        "rounded-md px-2 py-0.5 text-[11px] font-medium",
        tone === "accent"
          ? "bg-[var(--accent-glow)] text-content"
          : "bg-surface-3 text-content-muted",
      )}
    >
      {children}
    </span>
  );
}

function InfoSidebar({ details }: { details: ProjectDetails }) {
  const environment = environmentLabel(details.client_side, details.server_side);
  const versions = details.game_versions.slice(0, 14);
  const more = details.game_versions.length - versions.length;

  return (
    <aside className="flex w-64 shrink-0 flex-col gap-3">
      {(versions.length > 0 || details.loaders.length > 0) && (
        <SideCard title="Compatibility">
          {versions.length > 0 && (
            <div className="flex flex-wrap gap-1.5">
              {versions.map((v) => (
                <Chip key={v}>{v}</Chip>
              ))}
              {more > 0 && <Chip>+{more}</Chip>}
            </div>
          )}
          {details.loaders.length > 0 && (
            <>
              <div className="mb-1.5 mt-3 text-xs text-content-faint">Platforms</div>
              <div className="flex flex-wrap gap-1.5">
                {details.loaders.map((l) => (
                  <Chip key={l} tone="accent">
                    {l}
                  </Chip>
                ))}
              </div>
            </>
          )}
          {environment && (
            <>
              <div className="mb-1.5 mt-3 text-xs text-content-faint">Environment</div>
              <Chip>{environment}</Chip>
            </>
          )}
        </SideCard>
      )}

      {details.links.length > 0 && (
        <SideCard title="Links">
          <div className="flex flex-col gap-1">
            {details.links.map((link) => (
              <button
                key={link.url}
                onClick={() => openUrl(link.url)}
                className="flex items-center justify-between rounded-lg px-2 py-1.5 text-left text-xs font-medium text-content-muted transition-colors hover:bg-surface-3 hover:text-content"
              >
                {link.label}
                <ExternalLink className="size-3 text-content-faint" />
              </button>
            ))}
          </div>
        </SideCard>
      )}

      {details.categories.length > 0 && (
        <SideCard title="Tags">
          <div className="flex flex-wrap gap-1.5">
            {details.categories.map((c) => (
              <Chip key={c}>{c}</Chip>
            ))}
          </div>
        </SideCard>
      )}

      <SideCard title="Details">
        <div className="flex flex-col gap-1.5 text-xs text-content-muted">
          {details.license && <div>Licensed {details.license}</div>}
          {details.published && (
            <div>
              Published {relativeTime(Math.floor(new Date(details.published).getTime() / 1000))}
            </div>
          )}
          {details.updated && (
            <div>
              Updated {relativeTime(Math.floor(new Date(details.updated).getTime() / 1000))}
            </div>
          )}
        </div>
      </SideCard>
    </aside>
  );
}

export function ProjectView() {
  const projectRef = useStore((s) => s.projectRef);
  const kind = useStore((s) => s.searchKind);
  const instance = useStore((s) => s.instances.find((i) => i.id === s.detailInstanceId));
  const goBack = useStore((s) => s.goBack);
  const openProject = useStore((s) => s.openProject);

  const [tab, setTab] = useState<Tab>("description");
  const [details, setDetails] = useState<ProjectDetails | null>(null);
  const [versions, setVersions] = useState<ProjectVersion[] | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [installing, setInstalling] = useState<string | null>(null);
  const [installed, setInstalled] = useState<Set<string>>(new Set());
  const [channel, setChannel] = useState<Channel>("all");
  const [compatibleOnly, setCompatibleOnly] = useState(true);
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [changelogs, setChangelogs] = useState<Record<string, Changelog | "loading">>({});
  const [gvFilter, setGvFilter] = useState<string>("all");
  const [loaderFilter, setLoaderFilter] = useState<string>("all");
  const [sortBy, setSortBy] = useState<"newest" | "downloads">("newest");
  const [visible, setVisible] = useState(PAGE_SIZE);
  const [installedFile, setInstalledFile] = useState<InstalledFile | null>(null);
  const [resolvedProjects, setResolvedProjects] = useState<Record<string, SearchResult | null>>({});
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    if (!projectRef) return;
    let live = true;
    setLoading(true);
    setDetails(null);
    setVersions(null);
    setInstalled(new Set());
    setTab("description");
    setError(null);
    setExpandedId(null);
    setChangelogs({});
    setGvFilter("all");
    setLoaderFilter("all");
    setSortBy("newest");
    setVisible(PAGE_SIZE);
    setResolvedProjects({});
    setNotice(null);
    api
      .getProjectDetails(projectRef.provider, projectRef.id)
      .then((d) => live && setDetails(d))
      .catch((e) => live && setError(String(e)))
      .finally(() => live && setLoading(false));
    return () => {
      live = false;
    };
  }, [projectRef?.provider, projectRef?.id]);

  useEffect(() => {
    if (!projectRef || !instance || !kind) return;
    let live = true;
    api
      .getInstalledProjectFile(instance.id, kind, projectRef.id)
      .then((f) => live && setInstalledFile(f))
      .catch(() => live && setInstalledFile(null));
    return () => {
      live = false;
    };
  }, [projectRef?.id, instance?.id, kind]);

  const loader = kind === "mods" ? (instance?.loader ?? null) : null;

  useEffect(() => {
    if (tab !== "versions" || versions !== null || !projectRef || !instance || !kind) return;
    let live = true;
    api
      .listProjectVersions(projectRef.provider, projectRef.id, kind, instance.version_id, loader)
      .then((v) => live && setVersions(v))
      .catch((e) => {
        if (live) {
          setVersions([]);
          setError(String(e));
        }
      });
    return () => {
      live = false;
    };
  }, [tab, versions, projectRef?.id, instance?.id, kind, loader]);

  if (!projectRef || !instance || !kind) {
    return (
      <div className="grid flex-1 place-items-center text-sm text-content-muted">
        No project selected.
      </div>
    );
  }

  const refreshInstalled = () => {
    api
      .getInstalledProjectFile(instance.id, kind, projectRef.id)
      .then(setInstalledFile)
      .catch(() => {});
  };

  const install = async (versionId: string | null) => {
    const key = versionId ?? "latest";
    setInstalling(key);
    setError(null);
    setNotice(null);
    try {
      const files = await api.installContent(
        projectRef.provider,
        projectRef.id,
        instance.id,
        kind,
        instance.version_id,
        loader,
        versionId,
        details?.title ?? null,
        details?.icon_url ?? null,
      );
      setInstalled((prev) => new Set(prev).add(key));
      setNotice(
        files.length > 1
          ? `Installed ${files[0]} and ${files.length - 1} ${files.length === 2 ? "dependency" : "dependencies"} (${files.slice(1).join(", ")})`
          : `Installed ${files[0]}`,
      );
      refreshInstalled();
    } catch (e) {
      setError(String(e));
    } finally {
      setInstalling(null);
    }
  };

  const installDependency = async (dep: SearchResult) => {
    setInstalling(`dep:${dep.id}`);
    setError(null);
    try {
      const files = await api.installContent(
        projectRef.provider,
        dep.id,
        instance.id,
        kind,
        instance.version_id,
        loader,
        null,
        dep.title,
        dep.icon_url,
      );
      setInstalled((prev) => new Set(prev).add(`dep:${dep.id}`));
      setNotice(`Installed ${files.join(", ")}`);
    } catch (e) {
      setError(String(e));
    } finally {
      setInstalling(null);
    }
  };

  const toggleExpand = async (v: ProjectVersion) => {
    if (expandedId === v.id) {
      setExpandedId(null);
      return;
    }
    setExpandedId(v.id);

    const unresolved = v.dependencies
      .map((d) => d.project_id)
      .filter((id) => !(id in resolvedProjects));
    if (unresolved.length > 0) {
      setResolvedProjects((prev) => {
        const next = { ...prev };
        unresolved.forEach((id) => (next[id] = null));
        return next;
      });
      api
        .resolveProjects(projectRef.provider, unresolved)
        .then((results) => {
          setResolvedProjects((prev) => {
            const next = { ...prev };
            results.forEach((r) => (next[r.id] = r));
            return next;
          });
        })
        .catch(() => {});
    }

    if (changelogs[v.id]) return;
    if (v.changelog) {
      setChangelogs((prev) => ({
        ...prev,
        [v.id]: { body: v.changelog!, format: "markdown" },
      }));
      return;
    }
    setChangelogs((prev) => ({ ...prev, [v.id]: "loading" }));
    try {
      const changelog = await api.getVersionChangelog(
        projectRef.provider,
        projectRef.id,
        v.id,
      );
      setChangelogs((prev) => ({ ...prev, [v.id]: changelog }));
    } catch {
      setChangelogs((prev) => ({
        ...prev,
        [v.id]: { body: "", format: "markdown" },
      }));
    }
  };

  const gallery = details?.gallery ?? [];
  const tabs: Array<{ id: Tab; label: string }> = [
    { id: "description", label: "Description" },
    { id: "versions", label: "Versions" },
    ...(gallery.length > 0 ? [{ id: "gallery" as Tab, label: "Gallery" }] : []),
  ];

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex items-center gap-3 border-b border-border-soft px-6 py-4">
        <button
          onClick={goBack}
          aria-label="Back"
          className="grid size-9 shrink-0 place-items-center rounded-full border border-border bg-surface-2 text-content-muted transition-colors hover:bg-surface-3 hover:text-content"
        >
          <ArrowLeft className="size-4" />
        </button>

        {details?.icon_url ? (
          <img
            src={details.icon_url}
            className="size-12 shrink-0 rounded-xl bg-surface-2 object-cover"
            draggable={false}
          />
        ) : (
          <div className="grid size-12 shrink-0 place-items-center rounded-xl bg-surface-2 text-content-faint">
            <Package className="size-5" />
          </div>
        )}

        <div className="min-w-0 flex-1">
          <h1 className="truncate font-display text-lg font-semibold text-content">
            {details?.title ?? "Loading"}
          </h1>
          <div className="flex items-center gap-3 text-xs text-content-muted">
            {details?.author && (
              <span className="inline-flex items-center gap-1">
                <User className="size-3" />
                {details.author}
              </span>
            )}
            {details && (
              <span className="inline-flex items-center gap-1">
                <Download className="size-3" />
                {formatDownloads(details.downloads)}
              </span>
            )}
            <span className="rounded bg-surface-2 px-1.5 py-0.5 text-[10px] uppercase tracking-wide text-content-faint">
              {projectRef.provider}
            </span>
          </div>
        </div>

        <button
          onClick={() => install(null)}
          disabled={installing !== null || installed.has("latest") || loading}
          className={cn(
            "inline-flex h-10 shrink-0 items-center gap-2 rounded-xl px-5 text-sm font-semibold transition-all",
            installed.has("latest")
              ? "cursor-default bg-ok/15 text-ok"
              : "text-black shadow-lg shadow-[var(--accent-glow)] [background:linear-gradient(to_bottom,var(--accent),var(--accent-deep))] hover:[background:linear-gradient(to_bottom,var(--accent-bright),var(--accent))] disabled:opacity-60",
          )}
        >
          {installed.has("latest") ? (
            <>
              <Check className="size-4" />
              Added to {instance.name}
            </>
          ) : installing === "latest" ? (
            <Loader2 className="size-4 animate-spin" />
          ) : (
            <>
              <Download className="size-4" />
              Install latest
            </>
          )}
        </button>
      </div>

      <div className="flex gap-1 border-b border-border-soft px-6">
        {tabs.map((t) => (
          <button
            key={t.id}
            onClick={() => setTab(t.id)}
            className={cn(
              "relative px-4 py-2.5 text-sm font-medium transition-colors",
              tab === t.id ? "text-content" : "text-content-faint hover:text-content-muted",
            )}
          >
            {t.label}
            {tab === t.id && (
              <span className="absolute inset-x-2 -bottom-px h-0.5 rounded-full bg-[var(--accent)] transition-colors duration-500" />
            )}
          </button>
        ))}
      </div>

      {error && (
        <div className="mx-6 mt-3 flex items-start gap-2 rounded-lg border border-warn/30 bg-warn/10 px-3 py-2 text-xs text-warn">
          <TriangleAlert className="mt-0.5 size-3.5 shrink-0" />
          <span className="break-words">{error}</span>
        </div>
      )}
      {notice && tab !== "versions" && (
        <div className="mx-6 mt-3 rounded-lg border border-ok/30 bg-ok/10 px-3 py-2 text-xs text-ok">
          {notice}
        </div>
      )}

      <div className="min-h-0 flex-1 overflow-y-auto">
        {loading ? (
          <div className="flex items-center justify-center gap-2 py-16 text-sm text-content-muted">
            <Loader2 className="size-4 animate-spin" />
            Loading project
          </div>
        ) : tab === "description" && details ? (
          <div className="mx-auto flex max-w-5xl items-start gap-6 px-6 py-6">
            <div className="min-w-0 flex-1">
              {details.body_format === "markdown" ? (
                <div className="prose-basalt">
                  <ReactMarkdown
                    remarkPlugins={[remarkGfm]}
                    rehypePlugins={[rehypeRaw, [rehypeSanitize, sanitizeSchema]]}
                  >
                    {details.body}
                  </ReactMarkdown>
                </div>
              ) : (
                <div
                  className="prose-basalt"
                  dangerouslySetInnerHTML={{ __html: DOMPurify.sanitize(details.body) }}
                />
              )}
            </div>
            <InfoSidebar details={details} />
          </div>
        ) : tab === "versions" ? (
          <div className="mx-auto max-w-4xl px-6 py-4">
            {versions === null ? (
              <div className="flex items-center justify-center gap-2 py-12 text-sm text-content-muted">
                <Loader2 className="size-4 animate-spin" />
                Loading versions
              </div>
            ) : (
              (() => {
                const filtered = versions.filter(
                  (v) =>
                    (channel === "all" || v.channel === channel) &&
                    (!compatibleOnly || v.compatible) &&
                    (gvFilter === "all" || v.game_versions.includes(gvFilter)) &&
                    (loaderFilter === "all" || v.loaders.includes(loaderFilter)),
                );
                const sorted =
                  sortBy === "downloads"
                    ? [...filtered].sort((a, b) => b.downloads - a.downloads)
                    : filtered;
                const shown = sorted.slice(0, visible);
                const compatibleCount = versions.filter((v) => v.compatible).length;
                const installedRow = versions.find((x) => x.id === installedFile?.version_id);
                const installedDate = installedRow ? new Date(installedRow.date).getTime() : null;
                const gvOptions: string[] = [];
                const loaderOptions: string[] = [];
                for (const v of versions) {
                  for (const g of v.game_versions) {
                    if (!gvOptions.includes(g)) gvOptions.push(g);
                  }
                  for (const l of v.loaders) {
                    if (!loaderOptions.includes(l)) loaderOptions.push(l);
                  }
                }
                return (
                  <>
                    <div className="mb-4 flex flex-wrap items-end gap-x-5 gap-y-3">
                      <div>
                        <div className="mb-1.5 text-[10px] font-semibold uppercase tracking-wider text-content-faint">
                          Game version
                        </div>
                        <div className="w-48">
                          <Select
                            value={gvFilter === "all" ? "All game versions" : gvFilter}
                            options={["All game versions", ...gvOptions]}
                            onChange={(v) => setGvFilter(v === "All game versions" ? "all" : v)}
                          />
                        </div>
                      </div>
                      {kind === "mods" && loaderOptions.length > 1 && (
                        <div>
                          <div className="mb-1.5 text-[10px] font-semibold uppercase tracking-wider text-content-faint">
                            Loader
                          </div>
                          <div className="w-40">
                            <Select
                              value={loaderFilter === "all" ? "All loaders" : loaderFilter}
                              options={["All loaders", ...loaderOptions]}
                              onChange={(v) => setLoaderFilter(v === "All loaders" ? "all" : v)}
                            />
                          </div>
                        </div>
                      )}
                      <div>
                        <div className="mb-1.5 text-[10px] font-semibold uppercase tracking-wider text-content-faint">
                          Channel
                        </div>
                        <div className="flex rounded-lg border border-border bg-surface-2 p-0.5">
                          {(["all", "release", "beta", "alpha"] as Channel[]).map((c) => (
                            <button
                              key={c}
                              onClick={() => setChannel(c)}
                              className={cn(
                                "rounded-md px-3 py-1.5 text-xs font-medium capitalize transition-colors",
                                channel === c
                                  ? "bg-surface-3 text-content"
                                  : "text-content-faint hover:text-content-muted",
                              )}
                            >
                              {c}
                            </button>
                          ))}
                        </div>
                      </div>
                      <div>
                        <div className="mb-1.5 text-[10px] font-semibold uppercase tracking-wider text-content-faint">
                          Sort
                        </div>
                        <div className="flex rounded-lg border border-border bg-surface-2 p-0.5">
                          {(
                            [
                              { id: "newest", label: "Newest" },
                              { id: "downloads", label: "Popular" },
                            ] as const
                          ).map((s) => (
                            <button
                              key={s.id}
                              onClick={() => setSortBy(s.id)}
                              className={cn(
                                "rounded-md px-3 py-1.5 text-xs font-medium transition-colors",
                                sortBy === s.id
                                  ? "bg-surface-3 text-content"
                                  : "text-content-faint hover:text-content-muted",
                              )}
                            >
                              {s.label}
                            </button>
                          ))}
                        </div>
                      </div>
                      <div>
                        <div className="mb-1.5 text-[10px] font-semibold uppercase tracking-wider text-content-faint">
                          Compatibility
                        </div>
                        <button
                          onClick={() => setCompatibleOnly((v) => !v)}
                          className={cn(
                            "inline-flex items-center gap-1.5 rounded-lg border px-3 py-[7px] text-xs font-medium transition-colors",
                            compatibleOnly
                              ? "border-ok/40 bg-ok/10 text-ok"
                              : "border-border bg-surface-2 text-content-muted hover:text-content",
                          )}
                        >
                          <Check className={cn("size-3.5", !compatibleOnly && "opacity-30")} />
                          {instance.version_id}
                          {loader && ` · ${loader}`}
                        </button>
                      </div>
                      <div className="ml-auto self-end pb-1.5 text-xs text-content-faint">
                        {shown.length} of {sorted.length} shown · {compatibleCount} compatible ·{" "}
                        {versions.length} total
                      </div>
                    </div>

                    {notice && (
                      <div className="mb-3 rounded-lg border border-ok/30 bg-ok/10 px-3 py-2 text-xs text-ok">
                        {notice}
                      </div>
                    )}

                    {shown.length === 0 ? (
                      <div className="py-12 text-center text-sm text-content-faint">
                        Nothing matches these filters.
                      </div>
                    ) : (
                      <div className="flex flex-col gap-1.5">
                        {shown.map((v) => {
                          const done = installed.has(v.id);
                          const busy = installing === v.id;
                          const expanded = expandedId === v.id;
                          const changelog = changelogs[v.id];
                          const isInstalledRow = installedFile?.version_id === v.id;
                          const isUpdate =
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
                                  ? "border-[var(--accent)] bg-[var(--accent-glow)]"
                                  : v.compatible
                                    ? "border-ok/35 bg-ok/[0.05]"
                                    : "border-border-soft bg-surface-2/40",
                              )}
                            >
                              <div
                                className={cn(
                                  "grid cursor-pointer grid-cols-[4.5rem_minmax(0,1fr)_auto_auto_auto] items-center gap-3 px-4 py-2.5",
                                  !v.compatible && !isInstalledRow && "opacity-60",
                                )}
                                onClick={() => toggleExpand(v)}
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
                                    <span className="truncate text-sm font-medium text-content">
                                      {v.name}
                                    </span>
                                    {isInstalledRow && (
                                      <span className="shrink-0 rounded bg-[var(--accent)] px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-wide text-black">
                                        Installed
                                      </span>
                                    )}
                                    {v.dependencies.length > 0 && (
                                      <span className="shrink-0 text-[10px] text-content-faint">
                                        {v.dependencies.length} dep
                                        {v.dependencies.length > 1 ? "s" : ""}
                                      </span>
                                    )}
                                  </div>
                                  <div className="mt-0.5 flex items-center gap-1.5">
                                    {v.game_versions.slice(0, 3).map((g) => (
                                      <span
                                        key={g}
                                        className={cn(
                                          "rounded bg-surface-3 px-1.5 py-0.5 text-[10px] font-medium",
                                          g === instance.version_id
                                            ? "bg-ok/20 text-ok"
                                            : "text-content-faint",
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
                                        className="rounded bg-[var(--accent-glow)] px-1.5 py-0.5 text-[10px] font-medium capitalize text-content-muted"
                                      >
                                        {l}
                                      </span>
                                    ))}
                                  </div>
                                </div>
                                <div className="hidden text-right text-[11px] leading-tight text-content-faint sm:block">
                                  <div>{formatDownloads(v.downloads)} downloads</div>
                                  <div>
                                    {v.size != null && `${formatSize(v.size)} · `}
                                    {v.date &&
                                      relativeTime(Math.floor(new Date(v.date).getTime() / 1000))}
                                  </div>
                                </div>
                                {isInstalledRow ? (
                                  <span className="inline-flex h-8 shrink-0 items-center gap-1.5 rounded-lg bg-ok/15 px-3 text-xs font-semibold text-ok">
                                    <Check className="size-3.5" />
                                    Current
                                  </span>
                                ) : (
                                  <button
                                    onClick={(e) => {
                                      e.stopPropagation();
                                      install(v.id);
                                    }}
                                    disabled={busy || done || installing !== null || !v.compatible}
                                    title={
                                      v.compatible
                                        ? undefined
                                        : "Not compatible with this instance"
                                    }
                                    className={cn(
                                      "inline-flex h-8 shrink-0 items-center gap-1.5 rounded-lg px-3 text-xs font-semibold transition-all",
                                      done
                                        ? "cursor-default bg-ok/15 text-ok"
                                        : isUpdate
                                          ? "bg-warn/15 text-warn hover:bg-warn/25 disabled:opacity-50"
                                          : v.compatible
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
                                            "rounded bg-surface-3 px-1.5 py-0.5 text-[10px] font-medium",
                                            g === instance.version_id
                                              ? "bg-ok/20 text-ok"
                                              : "text-content-muted",
                                          )}
                                        >
                                          {g}
                                        </span>
                                      ))}
                                      {v.loaders.map((l) => (
                                        <span
                                          key={l}
                                          className="rounded bg-[var(--accent-glow)] px-1.5 py-0.5 text-[10px] font-medium capitalize text-content-muted"
                                        >
                                          {l}
                                        </span>
                                      ))}
                                    </div>
                                  </div>
                                  {v.dependencies.length > 0 && (
                                    <div className="mb-3">
                                      <div className="mb-1.5 text-[11px] font-semibold uppercase tracking-wide text-content-faint">
                                        Dependencies
                                      </div>
                                      <div className="flex flex-col gap-1">
                                        {v.dependencies.map((dep) => {
                                          const info = resolvedProjects[dep.project_id];
                                          const depKey = `dep:${dep.project_id}`;
                                          const depBusy = installing === depKey;
                                          const depDone = installed.has(depKey);
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
                                                onClick={() =>
                                                  openProject(
                                                    projectRef.provider,
                                                    dep.project_id,
                                                    kind,
                                                  )
                                                }
                                                className="min-w-0 truncate text-left text-xs font-medium text-content hover:underline"
                                              >
                                                {info?.title ?? dep.project_id}
                                              </button>
                                              <span
                                                className={cn(
                                                  "shrink-0 rounded px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-wide",
                                                  DEP_STYLE[dep.dependency_type] ??
                                                    DEP_STYLE.optional,
                                                )}
                                              >
                                                {dep.dependency_type}
                                              </span>
                                              {(dep.dependency_type === "required" ||
                                                dep.dependency_type === "optional") &&
                                                info && (
                                                  <button
                                                    onClick={() => installDependency(info)}
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
                                  ) : (
                                    <ChangelogBody changelog={changelog} />
                                  )}
                                  <div className="mt-3 flex items-center gap-3 border-t border-border-soft pt-2 text-[10px] text-content-faint">
                                    <span>{v.version_number}</span>
                                    {v.date && <span>{new Date(v.date).toLocaleString()}</span>}
                                    {details?.website_url && (
                                      <button
                                        onClick={() =>
                                          openUrl(
                                            projectRef.provider === "modrinth"
                                              ? `${details.website_url}/version/${v.id}`
                                              : `${details.website_url}/files/${v.id}`,
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
              })()
            )}
          </div>
        ) : tab === "gallery" ? (
          <div className="mx-auto grid max-w-4xl grid-cols-[repeat(auto-fill,minmax(280px,1fr))] gap-4 px-6 py-6">
            {gallery.map((url) => (
              <img
                key={url}
                src={url}
                className="w-full rounded-xl border border-border-soft object-cover"
                draggable={false}
              />
            ))}
          </div>
        ) : null}
      </div>
    </div>
  );
}
