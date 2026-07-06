import { useEffect, useRef, useState } from "react";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import {
  ArrowLeft,
  Check,
  ChevronRight,
  Download,
  FileUp,
  Loader2,
  Package,
  Search,
  TriangleAlert,
} from "lucide-react";

import { cn } from "../lib/cn";
import { api } from "../lib/api";
import type { SearchProvider, SearchResult } from "../lib/types";
import { DependencyPrompt } from "../components/DependencyPrompt";
import { useStore } from "../store";

const PROVIDERS: Array<{ id: SearchProvider; label: string }> = [
  { id: "modrinth", label: "Modrinth" },
  { id: "curseforge", label: "CurseForge" },
];

const KIND_LABEL: Record<string, string> = {
  mods: "mods",
  resourcepacks: "resource packs",
  shaderpacks: "shaders",
};

const KIND_EXTENSIONS: Record<string, string[]> = {
  mods: ["jar"],
  resourcepacks: ["zip"],
  shaderpacks: ["zip"],
};

export function formatDownloads(count: number): string {
  if (count >= 1_000_000) return `${(count / 1_000_000).toFixed(1)}M`;
  if (count >= 1_000) return `${(count / 1_000).toFixed(0)}K`;
  return `${count}`;
}

export function SearchView() {
  const kind = useStore((s) => s.searchKind);
  const instance = useStore((s) => s.instances.find((i) => i.id === s.detailInstanceId));
  const goBack = useStore((s) => s.goBack);
  const openProject = useStore((s) => s.openProject);

  const [provider, setProvider] = useState<SearchProvider>("modrinth");
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);
  const [searching, setSearching] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const installingContent = useStore((s) => s.installingContent);
  const sources = useStore(
    (s) => s.contentSources[`${s.detailInstanceId}:${s.searchKind}`],
  );
  const refreshContentSources = useStore((s) => s.refreshContentSources);
  const installContent = useStore((s) => s.installContent);

  const [pending, setPending] = useState<{
    result: SearchResult;
    deps: SearchResult[];
  } | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  useEffect(() => {
    if (instance && kind) void refreshContentSources(instance.id, kind);
  }, [instance?.id, kind, refreshContentSources]);

  const loader = kind === "mods" ? (instance?.loader ?? null) : null;

  useEffect(() => {
    if (!instance || !kind) return;
    setSearching(true);
    clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(async () => {
      try {
        const hits = await api.searchContent(provider, kind, query, instance.version_id, loader);
        setResults(hits);
        setError(null);
      } catch (e) {
        setResults([]);
        setError(String(e));
      } finally {
        setSearching(false);
      }
    }, 350);
    return () => clearTimeout(debounceRef.current);
  }, [provider, query, kind, instance?.id, loader]);

  if (!instance || !kind) {
    return (
      <div className="grid flex-1 place-items-center text-sm text-content-muted">
        Nothing to search for.
      </div>
    );
  }

  const doInstall = async (result: SearchResult, withDependencies: boolean) => {
    setError(null);
    setNotice(null);
    setPending(null);
    try {
      const files = await installContent({
        provider,
        projectId: result.id,
        instanceId: instance.id,
        kind,
        gameVersion: instance.version_id,
        loader,
        title: result.title,
        iconUrl: result.icon_url,
        withDependencies,
      });
      setNotice(
        files.length > 1
          ? `Added ${files[0]} and ${files.length - 1} ${files.length === 2 ? "dependency" : "dependencies"} (${files.slice(1).join(", ")})`
          : `Added ${files[0]}`,
      );
    } catch (err) {
      setError(String(err));
    }
  };

  const install = async (result: SearchResult, e: React.MouseEvent) => {
    e.stopPropagation();
    setError(null);
    try {
      const missing = await api.getMissingDependencies(
        provider,
        result.id,
        instance.id,
        kind,
        instance.version_id,
        loader,
      );
      if (missing.length > 0) {
        setPending({ result, deps: missing });
        return;
      }
    } catch {
      void 0;
    }
    await doInstall(result, true);
  };

  const addFromFile = async () => {
    const files = await openFileDialog({
      multiple: true,
      directory: false,
      filters: [{ name: kind, extensions: KIND_EXTENSIONS[kind] ?? ["*"] }],
    });
    if (!files) return;
    const sources = Array.isArray(files) ? files : [files];
    await api.addInstanceContent(instance.id, kind, sources);
    goBack();
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
        <div className="min-w-0 flex-1">
          <h1 className="truncate font-display text-lg font-semibold text-content">
            Add {KIND_LABEL[kind] ?? kind}
          </h1>
          <div className="truncate text-xs text-content-muted">
            {instance.name} · {instance.version_id}
            {loader && ` · ${loader}`}
          </div>
        </div>
        <button
          onClick={addFromFile}
          className="inline-flex shrink-0 items-center gap-1.5 rounded-lg border border-border bg-surface-2 px-3 py-2 text-xs font-medium text-content-muted transition-colors hover:bg-surface-3 hover:text-content"
        >
          <FileUp className="size-3.5" />
          From file
        </button>
      </div>

      <div className="flex items-center gap-2 px-6 py-3">
        <div className="flex rounded-lg border border-border bg-surface-2 p-0.5">
          {PROVIDERS.map((p) => (
            <button
              key={p.id}
              onClick={() => setProvider(p.id)}
              className={cn(
                "rounded-md px-3 py-1.5 text-xs font-medium transition-colors",
                provider === p.id
                  ? "bg-surface-3 text-content"
                  : "text-content-faint hover:text-content-muted",
              )}
            >
              {p.label}
            </button>
          ))}
        </div>
        <div className="relative flex-1">
          <Search className="absolute left-3 top-1/2 size-4 -translate-y-1/2 text-content-faint" />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={`Search ${provider === "modrinth" ? "Modrinth" : "CurseForge"}`}
            autoFocus
            className="w-full rounded-lg border border-border bg-base py-2 pl-9 pr-3 text-sm text-content outline-none transition-colors focus:border-[var(--accent)]"
          />
        </div>
      </div>

      {error && (
        <div className="mx-6 mb-2 flex items-start gap-2 rounded-lg border border-warn/30 bg-warn/10 px-3 py-2 text-xs text-warn">
          <TriangleAlert className="mt-0.5 size-3.5 shrink-0" />
          <span className="break-words">{error}</span>
        </div>
      )}
      {notice && (
        <div className="mx-6 mb-2 rounded-lg border border-ok/30 bg-ok/10 px-3 py-2 text-xs text-ok">
          {notice}
        </div>
      )}

      <div className="min-h-0 flex-1 overflow-y-auto px-4 pb-4">
        {searching ? (
          <div className="flex items-center justify-center gap-2 py-16 text-sm text-content-muted">
            <Loader2 className="size-4 animate-spin" />
            Searching
          </div>
        ) : results.length === 0 ? (
          <div className="flex flex-col items-center gap-2 py-16 text-center text-sm text-content-faint">
            <Package className="size-6" />
            No results
          </div>
        ) : (
          results.map((result) => {
            const installedFile = sources?.[result.id]?.file_name;
            const done = !!installedFile;
            const busy = installingContent.includes(
              `${instance.id}:${kind}:${result.id}`,
            );
            return (
              <div
                key={result.id}
                onClick={() => openProject(provider, result.id)}
                className="group flex cursor-pointer items-center gap-3 rounded-xl px-3 py-3 transition-colors hover:bg-surface-2"
              >
                {result.icon_url ? (
                  <img
                    src={result.icon_url}
                    className="size-12 shrink-0 rounded-xl bg-surface-2 object-cover"
                    draggable={false}
                  />
                ) : (
                  <div className="grid size-12 shrink-0 place-items-center rounded-xl bg-surface-2 text-content-faint">
                    <Package className="size-5" />
                  </div>
                )}
                <div className="min-w-0 flex-1">
                  <div className="flex items-baseline gap-2">
                    <span className="truncate text-sm font-semibold text-content">
                      {result.title}
                    </span>
                    <span className="shrink-0 text-[11px] text-content-faint">
                      by {result.author} · {formatDownloads(result.downloads)} downloads
                    </span>
                  </div>
                  <div className="truncate text-xs text-content-muted">{result.description}</div>
                  {installedFile && (
                    <div className="mt-0.5 truncate text-[11px] text-ok">
                      Installed · {installedFile}
                    </div>
                  )}
                </div>
                <button
                  onClick={(e) => install(result, e)}
                  disabled={busy || done}
                  title={installedFile}
                  className={cn(
                    "inline-flex h-8 shrink-0 items-center gap-1.5 rounded-lg px-3 text-xs font-semibold transition-all",
                    done
                      ? "cursor-default bg-ok/15 text-ok"
                      : "text-black shadow-md shadow-[var(--accent-glow)] [background:linear-gradient(to_bottom,var(--accent),var(--accent-deep))] hover:[background:linear-gradient(to_bottom,var(--accent-bright),var(--accent))] disabled:opacity-70",
                  )}
                >
                  {done ? (
                    <>
                      <Check className="size-3.5" />
                      Installed
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
                <ChevronRight className="size-4 shrink-0 text-content-faint opacity-0 transition-opacity group-hover:opacity-100" />
              </div>
            );
          })
        )}
      </div>

      <DependencyPrompt
        prompt={pending ? { title: pending.result.title, deps: pending.deps } : null}
        busy={installingContent.length > 0}
        onInstallAll={() => pending && doInstall(pending.result, true)}
        onSkip={() => pending && doInstall(pending.result, false)}
        onCancel={() => setPending(null)}
      />
    </div>
  );
}
