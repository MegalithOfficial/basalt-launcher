import { useEffect, useState } from "react";
import DOMPurify from "dompurify";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import {
  ArrowLeft,
  Check,
  Download,
  Loader2,
  Package,
  TriangleAlert,
  User,
} from "lucide-react";

import { cn } from "../lib/cn";
import { api } from "../lib/api";
import type { ProjectDetails } from "../lib/types";
import { formatDownloads } from "./SearchView";
import { useStore } from "../store";

export function ProjectView() {
  const projectRef = useStore((s) => s.projectRef);
  const kind = useStore((s) => s.searchKind);
  const instance = useStore((s) => s.instances.find((i) => i.id === s.detailInstanceId));
  const goBack = useStore((s) => s.goBack);

  const [details, setDetails] = useState<ProjectDetails | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [installing, setInstalling] = useState(false);
  const [installed, setInstalled] = useState(false);

  useEffect(() => {
    if (!projectRef) return;
    let live = true;
    setLoading(true);
    setDetails(null);
    setInstalled(false);
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

  if (!projectRef || !instance || !kind) {
    return (
      <div className="grid flex-1 place-items-center text-sm text-content-muted">
        No project selected.
      </div>
    );
  }

  const loader = kind === "mods" ? instance.loader : null;

  const install = async () => {
    setInstalling(true);
    setError(null);
    try {
      await api.installContent(
        projectRef.provider,
        projectRef.id,
        instance.id,
        kind,
        instance.version_id,
        loader,
      );
      setInstalled(true);
    } catch (e) {
      setError(String(e));
    } finally {
      setInstalling(false);
    }
  };

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
          onClick={install}
          disabled={installing || installed || loading}
          className={cn(
            "inline-flex h-10 shrink-0 items-center gap-2 rounded-xl px-5 text-sm font-semibold transition-all",
            installed
              ? "cursor-default bg-ok/15 text-ok"
              : "text-black shadow-lg shadow-[var(--accent-glow)] [background:linear-gradient(to_bottom,var(--accent),var(--accent-deep))] hover:[background:linear-gradient(to_bottom,var(--accent-bright),var(--accent))] disabled:opacity-60",
          )}
        >
          {installed ? (
            <>
              <Check className="size-4" />
              Added to {instance.name}
            </>
          ) : installing ? (
            <Loader2 className="size-4 animate-spin" />
          ) : (
            <>
              <Download className="size-4" />
              Install
            </>
          )}
        </button>
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
        ) : details ? (
          <div className="mx-auto max-w-3xl px-6 py-6">
            {details.gallery.length > 0 && (
              <div className="mb-6 flex gap-3 overflow-x-auto pb-2">
                {details.gallery.slice(0, 8).map((url) => (
                  <img
                    key={url}
                    src={url}
                    className="h-40 shrink-0 rounded-xl border border-border-soft object-cover"
                    draggable={false}
                  />
                ))}
              </div>
            )}

            {details.body_format === "markdown" ? (
              <div className="prose-basalt">
                <ReactMarkdown remarkPlugins={[remarkGfm]}>{details.body}</ReactMarkdown>
              </div>
            ) : (
              <div
                className="prose-basalt"
                dangerouslySetInnerHTML={{ __html: DOMPurify.sanitize(details.body) }}
              />
            )}
          </div>
        ) : null}
      </div>
    </div>
  );
}
