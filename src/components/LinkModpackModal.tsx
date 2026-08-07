import { useEffect, useState } from "react";
import { ArrowLeft, Link2, Loader2, Package, Search, TriangleAlert } from "lucide-react";
import { toast } from "sonner";

import { api } from "../lib/api";
import { cn } from "../lib/cn";
import type { Instance, ProjectSummary, ProjectVersion, SearchProvider } from "../lib/types";
import { Modal, ModalBody, ModalFooter, ModalHeader } from "./Modal";
import { useStore } from "../store";

const PROVIDERS: Array<{ id: SearchProvider; label: string }> = [
  { id: "modrinth", label: "Modrinth" },
  { id: "curseforge", label: "CurseForge" },
];

export function LinkModpackModal({
  instance,
  open,
  onClose,
}: {
  instance: Instance;
  open: boolean;
  onClose: () => void;
}) {
  const refreshInstances = useStore((s) => s.refreshInstances);
  const hasCfKey = useStore((s) => !!s.settings?.curseforge_api_key || s.bundledCurseforgeKey);

  const [provider, setProvider] = useState<SearchProvider>("modrinth");
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<ProjectSummary[]>([]);
  const [searching, setSearching] = useState(false);
  const [picked, setPicked] = useState<ProjectSummary | null>(null);
  const [versions, setVersions] = useState<ProjectVersion[]>([]);
  const [loadingVersions, setLoadingVersions] = useState(false);
  const [linking, setLinking] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setQuery(instance.name);
    setPicked(null);
    setError(null);
  }, [open, instance.name]);

  useEffect(() => {
    if (!open || picked) return;
    let live = true;
    setSearching(true);
    const timer = setTimeout(() => {
      api
        .searchContent(provider, "modpacks", {
          query: query.trim(),
          game_versions: [],
          loaders: [],
          categories: [],
          environment: null,
          open_source_only: false,
          sort: "relevance",
          offset: 0,
          limit: 20,
        })
        .then((page) => live && setHits(page.hits))
        .catch(() => live && setHits([]))
        .finally(() => live && setSearching(false));
    }, 300);
    return () => {
      live = false;
      clearTimeout(timer);
    };
  }, [open, provider, query, picked]);

  useEffect(() => {
    if (!picked) return;
    let live = true;
    setLoadingVersions(true);
    api
      .listProjectVersions(provider, picked.id, "modpacks", "", null)
      .then((list) => live && setVersions(list))
      .catch(() => live && setVersions([]))
      .finally(() => live && setLoadingVersions(false));
    return () => {
      live = false;
    };
  }, [picked, provider]);

  const link = async (version: ProjectVersion) => {
    if (!picked) return;
    setLinking(version.id);
    setError(null);
    try {
      await api.linkModpack(instance.id, provider, picked.id, version.id);
      await refreshInstances();
      toast.success(`${instance.name} now follows ${picked.title}`, {
        description: `Tracking ${version.version_number}. Pack updates will show on the instance.`,
      });
      onClose();
    } catch (cause) {
      setError(String(cause));
    } finally {
      setLinking(null);
    }
  };

  return (
    <Modal
      open={open}
      onClose={onClose}
      size="wide"
      nested
      dismissable={linking === null}
      className="h-[min(620px,calc(100vh-48px))]"
      labelledBy="link-pack-title"
    >
      <ModalHeader
        id="link-pack-title"
        title={picked ? `Which version of ${picked.title}?` : "Link a modpack"}
        subtitle={
          picked
            ? "Pick the version this instance already matches, not the newest one."
            : `Follow a pack so ${instance.name} can receive its updates`
        }
        icon={
          <div className="grid size-9 place-items-center rounded-xl border border-border-soft bg-surface-2 text-(--accent)">
            <Link2 className="size-4" />
          </div>
        }
        onClose={linking === null ? onClose : undefined}
      />

      {!picked && (
        <div className="flex shrink-0 items-center gap-2 border-b border-border-soft px-5 py-3">
          <div className="flex shrink-0 rounded-lg border border-border-soft bg-surface-2/60 p-0.5">
            {PROVIDERS.map((entry) => (
              <button
                key={entry.id}
                onClick={() => setProvider(entry.id)}
                disabled={entry.id === "curseforge" && !hasCfKey}
                title={
                  entry.id === "curseforge" && !hasCfKey
                    ? "Add a CurseForge API key in Settings"
                    : undefined
                }
                className={cn(
                  "rounded-md px-3 py-1.5 text-xs font-medium transition-colors disabled:opacity-40",
                  provider === entry.id
                    ? "bg-surface-3 text-content"
                    : "text-content-faint hover:text-content-muted",
                )}
              >
                {entry.label}
              </button>
            ))}
          </div>
          <div className="relative min-w-0 flex-1">
            <Search className="absolute left-3 top-1/2 size-3.5 -translate-y-1/2 text-content-faint" />
            <input
              autoFocus
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder="Search modpacks"
              className="h-9 w-full rounded-lg border border-border bg-void pl-9 pr-3 text-sm text-content outline-none transition-colors placeholder:text-content-faint focus:border-(--accent)"
            />
          </div>
        </div>
      )}

      <ModalBody className="flex flex-col gap-2">
        {picked ? (
          loadingVersions ? (
            <div className="flex flex-1 items-center justify-center gap-2 text-sm text-content-muted">
              <Loader2 className="size-4 animate-spin" />
              Reading versions
            </div>
          ) : (
            versions.map((version) => (
              <button
                key={version.id}
                onClick={() => void link(version)}
                disabled={linking !== null}
                className="flex items-center gap-3 rounded-xl border border-border-soft bg-surface-2/50 px-3.5 py-3 text-left transition-colors hover:border-border hover:bg-surface-2 disabled:opacity-50"
              >
                <span className="min-w-0 flex-1">
                  <span className="block truncate text-sm font-medium text-content">
                    {version.version_number || version.name}
                  </span>
                  <span className="block truncate text-[11px] text-content-faint">
                    {version.game_versions.join(", ")}
                    {version.loaders.length > 0 ? ` · ${version.loaders.join(", ")}` : ""}
                    {` · ${version.channel}`}
                  </span>
                </span>
                {linking === version.id && (
                  <Loader2 className="size-4 shrink-0 animate-spin text-(--accent)" />
                )}
              </button>
            ))
          )
        ) : searching ? (
          <div className="flex flex-1 items-center justify-center gap-2 text-sm text-content-muted">
            <Loader2 className="size-4 animate-spin" />
            Searching
          </div>
        ) : hits.length === 0 ? (
          <div className="flex flex-1 items-center justify-center text-sm text-content-faint">
            Nothing matches that search.
          </div>
        ) : (
          hits.map((hit) => (
            <button
              key={hit.id}
              onClick={() => setPicked(hit)}
              className="flex items-center gap-3 rounded-xl border border-border-soft bg-surface-2/50 px-3.5 py-3 text-left transition-colors hover:border-border hover:bg-surface-2"
            >
              {hit.icon_url ? (
                <img
                  src={hit.icon_url}
                  alt=""
                  draggable={false}
                  className="size-10 shrink-0 rounded-lg bg-surface-3 object-cover"
                />
              ) : (
                <span className="grid size-10 shrink-0 place-items-center rounded-lg bg-surface-3 text-content-faint">
                  <Package className="size-4" />
                </span>
              )}
              <span className="min-w-0 flex-1">
                <span className="block truncate text-sm font-medium text-content">
                  {hit.title}
                </span>
                <span className="block truncate text-[11px] text-content-faint">
                  by {hit.author} · {hit.description}
                </span>
              </span>
            </button>
          ))
        )}

        {error && (
          <div className="flex gap-2.5 rounded-xl border border-danger/25 bg-danger/[0.07] px-3.5 py-3 text-xs text-danger">
            <TriangleAlert className="mt-0.5 size-4 shrink-0" />
            <span className="break-words">{error}</span>
          </div>
        )}
      </ModalBody>

      <ModalFooter className="justify-between">
        <span className="text-[11px] text-content-faint">
          Linking only records which pack this follows. No files are downloaded or removed.
        </span>
        <div className="flex items-center gap-2">
          {picked && (
            <button
              onClick={() => setPicked(null)}
              disabled={linking !== null}
              className="inline-flex items-center gap-1.5 rounded-lg px-3 py-2 text-sm font-medium text-content-muted transition-colors hover:text-content disabled:opacity-50"
            >
              <ArrowLeft className="size-3.5" />
              Back
            </button>
          )}
          <button
            onClick={onClose}
            disabled={linking !== null}
            className="rounded-lg px-3 py-2 text-sm font-medium text-content-muted transition-colors hover:text-content disabled:opacity-50"
          >
            Cancel
          </button>
        </div>
      </ModalFooter>
    </Modal>
  );
}
