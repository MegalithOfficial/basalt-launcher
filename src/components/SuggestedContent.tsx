import { useEffect, useState } from "react";
import { ArrowUpRight, Download, Loader2, Package } from "lucide-react";

import { api } from "../lib/api";
import { cn } from "../lib/cn";
import { formatCount } from "./ContentResults";
import type { ContentKind, Instance, ProjectSummary, SearchProvider } from "../lib/types";
import { useStore } from "../store";

const LIMIT = 5;
const SEARCHABLE: ContentKind[] = ["mods", "resourcepacks", "shaderpacks"];

interface Suggestion {
  provider: SearchProvider;
  project: ProjectSummary;
}

function Row({
  suggestion,
  busy,
  onInstall,
  onOpen,
}: {
  suggestion: Suggestion;
  busy: boolean;
  onInstall: () => void;
  onOpen: () => void;
}) {
  const { project, provider } = suggestion;
  return (
    <div className="flex items-center gap-3 rounded-xl border border-border-soft bg-surface-2/50 px-3.5 py-3 transition-colors hover:border-border">
      {project.icon_url ? (
        <img
          src={project.icon_url}
          alt=""
          className="size-10 shrink-0 rounded-lg bg-surface-3 object-cover"
          draggable={false}
        />
      ) : (
        <div className="grid size-10 shrink-0 place-items-center rounded-lg bg-surface-3 text-content-faint">
          <Package className="size-4" />
        </div>
      )}

      <div className="min-w-0 flex-1">
        <div className="flex min-w-0 items-center gap-2">
          <span className="truncate text-sm font-medium text-content">{project.title}</span>
          <span className="shrink-0 rounded bg-surface-3 px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-wide text-content-faint">
            {provider === "modrinth" ? "Modrinth" : "CurseForge"}
          </span>
        </div>
        <div className="truncate text-[11px] text-content-faint">
          by {project.author} · {formatCount(project.downloads)} downloads
        </div>
      </div>

      <button
        onClick={onOpen}
        title="Open the project page"
        className="inline-flex h-8 shrink-0 items-center gap-1.5 rounded-lg border border-border bg-surface-2 px-2.5 text-xs font-medium text-content-muted transition-colors hover:bg-surface-3 hover:text-content"
      >
        Details
        <ArrowUpRight className="size-3.5" />
      </button>
      <button
        onClick={onInstall}
        disabled={busy}
        className={cn(
          "inline-flex h-8 shrink-0 items-center gap-1.5 rounded-lg px-3 text-xs font-semibold text-black shadow-md shadow-(color:--accent-glow) transition-all",
          "[background:linear-gradient(to_bottom,var(--accent),var(--accent-deep))] hover:[background:linear-gradient(to_bottom,var(--accent-bright),var(--accent))]",
          busy && "cursor-not-allowed opacity-60",
        )}
      >
        {busy ? <Loader2 className="size-3.5 animate-spin" /> : <Download className="size-3.5" />}
        Install
      </button>
    </div>
  );
}

export function SuggestedContent({
  instance,
  kind,
  query,
  busyId,
  onInstall,
  onOpen,
}: {
  instance: Instance;
  kind: ContentKind;
  query: string;
  busyId: string | null;
  onInstall: (provider: SearchProvider, project: ProjectSummary) => void;
  onOpen: (provider: SearchProvider, project: ProjectSummary) => void;
}) {
  const hasCfKey = useStore((s) => !!s.settings?.curseforge_api_key || s.bundledCurseforgeKey);
  const installed = useStore((s) => s.contentSources[`${instance.id}:${kind}`]);
  const [results, setResults] = useState<Suggestion[]>([]);
  const [searching, setSearching] = useState(false);

  const needle = query.trim();
  const searchable = SEARCHABLE.includes(kind) && needle.length >= 2;

  useEffect(() => {
    if (!searchable) {
      setResults([]);
      return;
    }

    let live = true;
    setSearching(true);
    const timer = setTimeout(async () => {
      const providers: SearchProvider[] = hasCfKey ? ["modrinth", "curseforge"] : ["modrinth"];
      const query = {
        query: needle,
        game_versions: [instance.version_id],
        loaders: kind === "mods" && instance.loader ? [instance.loader] : [],
        categories: [],
        environment: null,
        open_source_only: false,
        sort: "relevance" as const,
        offset: 0,
        limit: LIMIT,
      };

      const pages = await Promise.all(
        providers.map((provider) =>
          api
            .searchContent(provider, kind, query)
            .then((page) => page.hits.slice(0, LIMIT).map((project) => ({ provider, project })))
            .catch(() => [] as Suggestion[]),
        ),
      );

      if (!live) return;
      setResults(pages.flat());
      setSearching(false);
    }, 350);

    return () => {
      live = false;
      clearTimeout(timer);
    };
  }, [searchable, needle, kind, instance.version_id, instance.loader, hasCfKey]);

  const shown = results.filter(({ project }) => !installed?.[project.id]);

  if (!searchable) return null;

  return (
    <div className="w-full border-t border-border-soft pt-6 text-left">
      <div className="mb-3.5 flex items-center gap-2 text-xs text-content-muted">
        {searching ? (
          <>
            <Loader2 className="size-3.5 animate-spin" />
            Looking for “{needle}” online
          </>
        ) : shown.length > 0 ? (
          <>
            <span className="font-medium text-content">Did you mean one of these?</span>
            <span className="text-content-faint">
              Compatible with {instance.version_id}
              {kind === "mods" && instance.loader ? ` · ${instance.loader}` : ""}
            </span>
          </>
        ) : (
          <>Nothing online matches “{needle}” for this instance either.</>
        )}
      </div>

      {!searching && shown.length > 0 && (
        <div className="flex flex-col gap-2 pb-8">
          {shown.map((suggestion) => (
            <Row
              key={`${suggestion.provider}:${suggestion.project.id}`}
              suggestion={suggestion}
              busy={busyId === suggestion.project.id}
              onInstall={() => onInstall(suggestion.provider, suggestion.project)}
              onOpen={() => onOpen(suggestion.provider, suggestion.project)}
            />
          ))}
        </div>
      )}
    </div>
  );
}
