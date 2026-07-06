import { useEffect, useRef, useState } from "react";
import { Check, Download, Loader2, Package, Search, TriangleAlert } from "lucide-react";

import { cn } from "../lib/cn";
import { api } from "../lib/api";
import type { SearchProvider, SearchResult } from "../lib/types";
import { ContentResults, ResultViewToggle, useResultView } from "../components/ContentResults";
import { formatDownloads } from "./SearchView";
import { useStore } from "../store";

const PROVIDERS: Array<{ id: SearchProvider; label: string }> = [
  { id: "modrinth", label: "Modrinth" },
  { id: "curseforge", label: "CurseForge" },
];

export function ModpacksView() {
  const installModpack = useStore((s) => s.installModpack);
  const openProject = useStore((s) => s.openProject);
  const openInstance = useStore((s) => s.openInstance);
  const instances = useStore((s) => s.instances);
  const hasCfKey = useStore((s) => !!s.settings?.curseforge_api_key);

  const [provider, setProvider] = useState<SearchProvider>("modrinth");
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);
  const [searching, setSearching] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [installing, setInstalling] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [resultView, setResultView] = useResultView("modpacks-view");
  const debounceRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  useEffect(() => {
    setSearching(true);
    clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(async () => {
      try {
        const hits = await api.searchContent(provider, "modpacks", query, "", null);
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
  }, [provider, query]);

  const installLatest = async (pack: SearchResult, e: React.MouseEvent) => {
    e.stopPropagation();
    setInstalling(pack.id);
    setError(null);
    setNotice(null);
    try {
      const versions = await api.listProjectVersions(provider, pack.id, "modpacks", "", null);
      const preferred = versions.find((v) => v.channel === "release") ?? versions[0];
      if (!preferred) {
        setError("This pack has no installable versions.");
        return;
      }
      const instance = await installModpack(provider, pack.id, preferred.id);
      setNotice(`Created instance ${instance.name}`);
    } catch (err) {
      setError(String(err));
    } finally {
      setInstalling(null);
    }
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="border-b border-border-soft px-6 py-6">
        <h1 className="font-display text-2xl font-semibold tracking-tight text-content">
          Modpacks
        </h1>
        <p className="mt-1 text-sm text-content-muted">
          Installing a pack creates a ready-to-play instance.
        </p>
      </div>

      <div className="flex items-center gap-2 px-6 py-3">
        <div className="flex rounded-lg border border-border bg-surface-2 p-0.5">
          {PROVIDERS.map((p) => {
            const disabled = p.id === "curseforge" && !hasCfKey;
            return (
              <button
                key={p.id}
                onClick={() => !disabled && setProvider(p.id)}
                disabled={disabled}
                title={disabled ? "Add a CurseForge API key in Settings" : undefined}
                className={cn(
                  "rounded-md px-3 py-1.5 text-xs font-medium transition-colors",
                  provider === p.id
                    ? "bg-surface-3 text-content"
                    : "text-content-faint hover:text-content-muted",
                  disabled && "cursor-not-allowed opacity-40",
                )}
              >
                {p.label}
              </button>
            );
          })}
        </div>
        <div className="relative flex-1">
          <Search className="absolute left-3 top-1/2 size-4 -translate-y-1/2 text-content-faint" />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search modpacks"
            autoFocus
            className="w-full rounded-lg border border-border bg-base py-2 pl-9 pr-3 text-sm text-content outline-none transition-colors focus:border-[var(--accent)]"
          />
        </div>
        <ResultViewToggle view={resultView} onChange={setResultView} />
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
          <ContentResults
            view={resultView}
            items={results.map((pack) => {
              const busy = installing === pack.id;
              const installedInstance = instances.find((i) => i.pack_project_id === pack.id);
              return {
                key: pack.id,
                iconUrl: pack.icon_url,
                title: pack.title,
                meta: `by ${pack.author} · ${formatDownloads(pack.downloads)} downloads`,
                description: pack.description,
                subline: installedInstance
                  ? `Installed as ${installedInstance.name}`
                  : undefined,
                onOpen: () => openProject(provider, pack.id, "modpacks"),
                action: installedInstance ? (
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      openInstance(installedInstance.id);
                    }}
                    className="inline-flex h-8 shrink-0 items-center gap-1.5 rounded-lg bg-ok/15 px-3 text-xs font-semibold text-ok transition-colors hover:bg-ok/25"
                  >
                    <Check className="size-3.5" />
                    Installed
                  </button>
                ) : (
                  <button
                    onClick={(e) => installLatest(pack, e)}
                    disabled={busy || installing !== null}
                    className="inline-flex h-8 shrink-0 items-center gap-1.5 rounded-lg px-3 text-xs font-semibold text-black shadow-md shadow-[var(--accent-glow)] transition-all [background:linear-gradient(to_bottom,var(--accent),var(--accent-deep))] hover:[background:linear-gradient(to_bottom,var(--accent-bright),var(--accent))] disabled:opacity-60"
                  >
                    {busy ? (
                      <Loader2 className="size-3.5 animate-spin" />
                    ) : (
                      <>
                        <Download className="size-3.5" />
                        Install
                      </>
                    )}
                  </button>
                ),
              };
            })}
          />
        )}
      </div>
    </div>
  );
}
