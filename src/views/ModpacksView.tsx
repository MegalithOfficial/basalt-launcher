import { useEffect, useRef, useState } from "react";
import {
  ChevronDown,
  Download,
  Loader2,
  Package,
  Search,
  TriangleAlert,
} from "lucide-react";

import { cn } from "../lib/cn";
import { api } from "../lib/api";
import { Select } from "../components/Select";
import type { ProjectVersion, SearchProvider, SearchResult } from "../lib/types";
import { formatDownloads } from "./SearchView";
import { useStore } from "../store";

const PROVIDERS: Array<{ id: SearchProvider; label: string }> = [
  { id: "modrinth", label: "Modrinth" },
  { id: "curseforge", label: "CurseForge" },
];

export function ModpacksView() {
  const setView = useStore((s) => s.setView);
  const installModpack = useStore((s) => s.installModpack);
  const hasCfKey = useStore((s) => !!s.settings?.curseforge_api_key);

  const [provider, setProvider] = useState<SearchProvider>("modrinth");
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);
  const [searching, setSearching] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [versions, setVersions] = useState<Record<string, ProjectVersion[] | "loading">>({});
  const [pickedVersion, setPickedVersion] = useState<Record<string, string>>({});
  const [installing, setInstalling] = useState<string | null>(null);
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

  const toggleExpand = (pack: SearchResult) => {
    if (expandedId === pack.id) {
      setExpandedId(null);
      return;
    }
    setExpandedId(pack.id);
    if (versions[pack.id]) return;
    setVersions((prev) => ({ ...prev, [pack.id]: "loading" }));
    api
      .listProjectVersions(provider, pack.id, "modpacks", "", null)
      .then((list) => {
        setVersions((prev) => ({ ...prev, [pack.id]: list }));
        const preferred = list.find((v) => v.channel === "release") ?? list[0];
        if (preferred) {
          setPickedVersion((prev) => ({ ...prev, [pack.id]: preferred.id }));
        }
      })
      .catch((e) => {
        setVersions((prev) => ({ ...prev, [pack.id]: [] }));
        setError(String(e));
      });
  };

  const install = async (pack: SearchResult) => {
    const versionId = pickedVersion[pack.id];
    if (!versionId) return;
    setInstalling(pack.id);
    setError(null);
    try {
      await installModpack(provider, pack.id, versionId);
      setView("home");
    } catch (e) {
      setError(String(e));
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
      </div>

      {error && (
        <div className="mx-6 mb-2 flex items-start gap-2 rounded-lg border border-warn/30 bg-warn/10 px-3 py-2 text-xs text-warn">
          <TriangleAlert className="mt-0.5 size-3.5 shrink-0" />
          <span className="break-words">{error}</span>
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
          results.map((pack) => {
            const expanded = expandedId === pack.id;
            const packVersions = versions[pack.id];
            const busy = installing === pack.id;
            return (
              <div
                key={pack.id}
                className={cn(
                  "mb-1.5 rounded-xl border transition-colors",
                  expanded ? "border-border bg-surface-2/60" : "border-transparent hover:bg-surface-2",
                )}
              >
                <div
                  onClick={() => toggleExpand(pack)}
                  className="flex cursor-pointer items-center gap-3 px-3 py-3"
                >
                  {pack.icon_url ? (
                    <img
                      src={pack.icon_url}
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
                        {pack.title}
                      </span>
                      <span className="shrink-0 text-[11px] text-content-faint">
                        by {pack.author} · {formatDownloads(pack.downloads)} downloads
                      </span>
                    </div>
                    <div className="truncate text-xs text-content-muted">{pack.description}</div>
                  </div>
                  <ChevronDown
                    className={cn(
                      "size-4 shrink-0 text-content-faint transition-transform",
                      expanded && "rotate-180",
                    )}
                  />
                </div>

                {expanded && (
                  <div className="flex items-center gap-2 border-t border-border-soft px-3 py-3">
                    {packVersions === "loading" || !packVersions ? (
                      <div className="flex items-center gap-2 text-xs text-content-muted">
                        <Loader2 className="size-3.5 animate-spin" />
                        Loading versions
                      </div>
                    ) : packVersions.length === 0 ? (
                      <div className="text-xs text-content-faint">No installable versions.</div>
                    ) : (
                      <>
                        <div className="w-72">
                          <Select
                            value={
                              packVersions.find((v) => v.id === pickedVersion[pack.id])?.name ??
                              null
                            }
                            options={packVersions.slice(0, 60).map((v) => v.name)}
                            onChange={(name) => {
                              const picked = packVersions.find((v) => v.name === name);
                              if (picked) {
                                setPickedVersion((prev) => ({ ...prev, [pack.id]: picked.id }));
                              }
                            }}
                            placeholder="Pick a version"
                          />
                        </div>
                        <button
                          onClick={() => install(pack)}
                          disabled={busy || installing !== null || !pickedVersion[pack.id]}
                          className="inline-flex h-9 items-center gap-2 rounded-lg px-4 text-xs font-semibold text-black shadow-md shadow-[var(--accent-glow)] transition-all [background:linear-gradient(to_bottom,var(--accent),var(--accent-deep))] hover:[background:linear-gradient(to_bottom,var(--accent-bright),var(--accent))] disabled:opacity-60"
                        >
                          {busy ? (
                            <>
                              <Loader2 className="size-3.5 animate-spin" />
                              Installing pack
                            </>
                          ) : (
                            <>
                              <Download className="size-3.5" />
                              Install
                            </>
                          )}
                        </button>
                      </>
                    )}
                  </div>
                )}
              </div>
            );
          })
        )}
      </div>
    </div>
  );
}
