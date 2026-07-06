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
import type { ProjectDetails, ProjectVersion } from "../lib/types";
import { formatDownloads } from "./SearchView";
import { useStore } from "../store";

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

  const [tab, setTab] = useState<Tab>("description");
  const [details, setDetails] = useState<ProjectDetails | null>(null);
  const [versions, setVersions] = useState<ProjectVersion[] | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [installing, setInstalling] = useState<string | null>(null);
  const [installed, setInstalled] = useState<Set<string>>(new Set());

  useEffect(() => {
    if (!projectRef) return;
    let live = true;
    setLoading(true);
    setDetails(null);
    setVersions(null);
    setInstalled(new Set());
    setTab("description");
    setError(null);
    api
      .getProjectDetails(projectRef.provider, projectRef.id)
      .then((d) => live && setDetails(d))
      .catch((e) => live && setError(String(e)))
      .finally(() => live && setLoading(false));
    return () => {
      live = false;
    };
  }, [projectRef?.provider, projectRef?.id]);

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

  const install = async (versionId: string | null) => {
    const key = versionId ?? "latest";
    setInstalling(key);
    setError(null);
    try {
      await api.installContent(
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
    } catch (e) {
      setError(String(e));
    } finally {
      setInstalling(null);
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
          <div className="mx-auto max-w-3xl px-6 py-4">
            {versions === null ? (
              <div className="flex items-center justify-center gap-2 py-12 text-sm text-content-muted">
                <Loader2 className="size-4 animate-spin" />
                Loading versions
              </div>
            ) : versions.length === 0 ? (
              <div className="py-12 text-center text-sm text-content-faint">
                No versions for {instance.version_id}
                {loader && ` on ${loader}`}.
              </div>
            ) : (
              <div className="flex flex-col gap-1.5">
                <div className="mb-1 px-1 text-xs text-content-faint">
                  <span className="mr-1.5 inline-block size-2 rounded-full bg-ok align-middle" />
                  Compatible with {instance.name} ({instance.version_id}
                  {loader && ` · ${loader}`})
                </div>
                {versions.map((v) => {
                  const done = installed.has(v.id);
                  const busy = installing === v.id;
                  return (
                    <div
                      key={v.id}
                      className={cn(
                        "flex items-center gap-3 rounded-xl border px-4 py-2.5 transition-colors",
                        v.compatible
                          ? "border-ok/35 bg-ok/[0.06]"
                          : "border-border-soft bg-surface-2/40 opacity-55",
                      )}
                    >
                      <span
                        className={cn(
                          "shrink-0 rounded px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide",
                          CHANNEL_STYLE[v.channel] ?? CHANNEL_STYLE.release,
                        )}
                      >
                        {v.channel}
                      </span>
                      <div className="min-w-0 flex-1">
                        <div className="flex items-center gap-2">
                          <span className="truncate text-sm font-medium text-content">
                            {v.name}
                          </span>
                          {v.game_versions.slice(0, 4).map((g) => (
                            <span
                              key={g}
                              className={cn(
                                "shrink-0 rounded bg-surface-3 px-1.5 py-0.5 text-[10px] font-medium",
                                g === instance.version_id
                                  ? "bg-ok/20 text-ok"
                                  : "text-content-faint",
                              )}
                            >
                              {g}
                            </span>
                          ))}
                          {v.game_versions.length > 4 && (
                            <span className="shrink-0 text-[10px] text-content-faint">
                              +{v.game_versions.length - 4}
                            </span>
                          )}
                        </div>
                        <div className="truncate text-[11px] text-content-faint">
                          {v.file_name}
                          {v.size != null && ` · ${formatSize(v.size)}`}
                          {v.date &&
                            ` · ${relativeTime(Math.floor(new Date(v.date).getTime() / 1000))}`}
                          {` · ${formatDownloads(v.downloads)} downloads`}
                        </div>
                      </div>
                      <button
                        onClick={() => install(v.id)}
                        disabled={busy || done || installing !== null || !v.compatible}
                        title={v.compatible ? undefined : "Not compatible with this instance"}
                        className={cn(
                          "inline-flex h-8 shrink-0 items-center gap-1.5 rounded-lg px-3 text-xs font-semibold transition-all",
                          done
                            ? "cursor-default bg-ok/15 text-ok"
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
                            Install
                          </>
                        )}
                      </button>
                    </div>
                  );
                })}
              </div>
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
