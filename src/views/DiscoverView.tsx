import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  Check,
  ChevronLeft,
  ChevronsLeft,
  ChevronsRight,
  ChevronRight,
  Download,
  Loader2,
  Package,
  Search,
  SlidersHorizontal,
  TriangleAlert,
} from "lucide-react";

import { cn } from "../lib/cn";
import { api } from "../lib/api";
import type {
  Instance,
  ContentKind,
  FilterTaxonomy,
  ProjectSummary,
  SearchPage,
  SearchProvider,
  SortOrder,
  WorldSummary,
} from "../lib/types";
import {
  ContentResults,
  ResultViewToggle,
  useResultView,
} from "../components/ContentResults";
import {
  countActive,
  emptyFilters,
  FilterRail,
  type FilterState,
} from "../components/FilterRail";
import { useContentInstaller } from "../components/CurseForgeDownloadModal";
import { Modal, ModalHeader } from "../components/Modal";
import { WorldTargetPicker } from "../components/WorldTargetPicker";
import { InstanceTargetPicker } from "../components/InstanceTargetPicker";
import { Select } from "../components/Select";
import {
  taskFraction,
  useActiveProjectIds,
  useActiveTasksByProject,
} from "../lib/useTasks";
import type { InstallTarget } from "../lib/target";
import { useStore } from "../store";

const KINDS: Array<{ id: ContentKind; label: string }> = [
  { id: "mods", label: "Mods" },
  { id: "modpacks", label: "Modpacks" },
  { id: "resourcepacks", label: "Resource Packs" },
  { id: "shaderpacks", label: "Shaders" },
  { id: "datapacks", label: "Datapacks" },
];

const PROVIDERS: Array<{ id: SearchProvider; label: string }> = [
  { id: "modrinth", label: "Modrinth" },
  { id: "curseforge", label: "CurseForge" },
];

const SORTS: Array<{ id: SortOrder; label: string }> = [
  { id: "relevance", label: "Relevance" },
  { id: "downloads", label: "Downloads" },
  { id: "follows", label: "Follows" },
  { id: "newest", label: "Newest" },
  { id: "updated", label: "Recently updated" },
];

const PAGE_SIZE = 40;



export function DiscoverView() {
  const kind = useStore((s) => s.discoverKind);
  const setKind = useStore((s) => s.setDiscoverKind);
  const targetId = useStore((s) => s.discoverTargetId);
  const setTarget = useStore((s) => s.setDiscoverTarget);
  const instances = useStore((s) => s.instances);
  const serverId = useStore((s) => s.discoverServerId);
  const servers = useStore((s) => s.servers);
  const serverSoftware = useStore((s) => s.serverSoftware);
  const setDiscoverServer = useStore((s) => s.setDiscoverServer);
  const contentServers = useMemo(
    () =>
      servers.filter(
        (entry) =>
          entry.available &&
          !!serverSoftware.find((spec) => spec.id === entry.flavor)?.content_dir,
      ),
    [servers, serverSoftware],
  );
  const target = useMemo<InstallTarget | null>(() => {
    if (serverId) {
      const server = servers.find((entry) => entry.id === serverId);
      return server
        ? {
            id: server.id,
            name: server.name,
            version_id: server.version_id,
            loader: server.flavor,
            isServer: true,
          }
        : null;
    }
    const instance = instances.find((entry) => entry.id === targetId);
    return instance
      ? {
          id: instance.id,
          name: instance.name,
          version_id: instance.version_id,
          loader: instance.loader,
          isServer: false,
        }
      : null;
  }, [instances, servers, targetId, serverId]);
  const openProject = useStore((s) => s.openProject);
  const openInstance = useStore((s) => s.openInstance);
  const openServer = useStore((s) => s.openServer);
  const refreshServers = useStore((s) => s.refreshServers);
  const activeProjects = useActiveProjectIds();
  const activeTasks = useActiveTasksByProject();
  const allSources = useStore((s) => s.contentSources);
  const sources = allSources[`${serverId ?? targetId}:${kind}`];
  const refreshContentSources = useStore((s) => s.refreshContentSources);
  const refreshServerContentSources = useStore((s) => s.refreshServerContentSources);
  const hasCfKey = useStore((s) => !!s.settings?.curseforge_api_key || s.bundledCurseforgeKey);

  const browse = useStore((s) => s.discoverBrowse);
  const setBrowse = useStore((s) => s.setDiscoverBrowse);
  const resetBrowse = useStore((s) => s.resetDiscoverBrowse);

  const { provider, query, sort, filters, offset, showFilters, page } = browse;
  const setProvider = (next: SearchProvider) => setBrowse({ provider: next });
  const setQuery = (next: string) => setBrowse({ query: next });
  const setSort = (next: SortOrder) => setBrowse({ sort: next });
  const setFilters = (next: FilterState | ((current: FilterState) => FilterState)) =>
    setBrowse({
      filters: typeof next === "function" ? next(browse.filters) : next,
    });
  const setPage = (next: SearchPage | null) => setBrowse({ page: next });
  const setOffset = (next: number) => setBrowse({ offset: next });
  const setShowFilters = (next: boolean | ((current: boolean) => boolean)) =>
    setBrowse({
      showFilters: typeof next === "function" ? next(browse.showFilters) : next,
    });

  const [taxonomy, setTaxonomy] = useState<FilterTaxonomy | null>(null);
  const [searching, setSearching] = useState(browse.page === null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [resultView, setResultView] = useResultView("discover-view");
  const [planning, setPlanning] = useState<string | null>(null);
  const [pickingWorld, setPickingWorld] = useState<{
    project: ProjectSummary;
    destination: InstallTarget;
    worlds: WorldSummary[];
  } | null>(null);
  const [worldFilter, setWorldFilter] = useState("");
  const pinnedWorld = useStore((s) => s.discoverWorld);
  const setPinnedWorld = useStore((s) => s.setDiscoverWorld);
  const [targetWorlds, setTargetWorlds] = useState<WorldSummary[]>([]);
  const [installingPack, setInstallingPack] = useState<string | null>(null);
  const [needsTarget, setNeedsTarget] = useState<ProjectSummary | null>(null);
  const contentInstaller = useContentInstaller();

  const debounceRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const requestRef = useRef(0);
  const scrollRef = useRef<HTMLDivElement>(null);

  const isPack = kind === "modpacks";
  const usesLoaders = kind === "mods" || kind === "modpacks";
  const modsBlocked = kind === "mods" && !!target && !target.loader;

  useEffect(() => {
    if (provider === "curseforge" && !hasCfKey) setProvider("modrinth");
  }, [provider, hasCfKey]);

  useEffect(() => {
    let live = true;
    setTaxonomy(null);
    api
      .getFilterTaxonomy(provider, kind)
      .then((t) => live && setTaxonomy(t))
      .catch(() => live && setTaxonomy(null));
    return () => {
      live = false;
    };
  }, [provider, kind]);

  const scope = `${provider}:${kind}`;

  useEffect(() => {
    if (browse.scope !== scope) resetBrowse({ provider, scope });
  }, [scope, browse.scope, provider, resetBrowse]);

  const seedKey = `${scope}:${targetId ?? ""}`;

  useEffect(() => {
    if (browse.seededFor === seedKey) return;
    if (isPack) {
      setBrowse({ seededFor: seedKey });
      return;
    }
    setBrowse({
      seededFor: seedKey,
      offset: 0,
      filters: target
        ? {
            ...emptyFilters,
            gameVersions: [target.version_id],
            loaders: usesLoaders && target.loader ? [target.loader] : [],
            environment: target.isServer ? "server" : emptyFilters.environment,
          }
        : emptyFilters,
    });
  }, [seedKey, browse.seededFor, target, isPack, usesLoaders]);

  const lastParams = useRef(JSON.stringify({ query, sort, filters }));

  useEffect(() => {
    const params = JSON.stringify({ query, sort, filters });
    if (lastParams.current === params) return;
    lastParams.current = params;
    setOffset(0);
  }, [query, sort, filters]);

  useEffect(() => {
    if (kind === "modpacks") return;
    if (target) {
      if (target.isServer) void refreshServerContentSources(target.id);
      else void refreshContentSources(target.id, kind);
      return;
    }
    for (const instance of instances) void refreshContentSources(instance.id, kind);
  }, [
    target?.id,
    target?.isServer,
    kind,
    instances,
    refreshContentSources,
    refreshServerContentSources,
  ]);

  const installedIn = useCallback(
    (projectId: string) =>
      instances.filter((instance) => !!allSources[`${instance.id}:${kind}`]?.[projectId]),
    [instances, allSources, kind],
  );

  const signature = JSON.stringify({ provider, kind, query, sort, filters, offset });
  const firstRun = useRef(true);

  useEffect(() => {
    const restored = firstRun.current && browse.page !== null && browse.signature === signature;
    firstRun.current = false;
    if (restored) {
      setSearching(false);
      return;
    }

    const ticket = ++requestRef.current;
    setSearching(true);
    clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(async () => {
      try {
        const result = await api.searchContent(provider, kind, {
          query,
          game_versions: filters.gameVersions,
          loaders: filters.loaders,
          categories: filters.categories,
          environment: filters.environment,
          open_source_only: filters.openSourceOnly,
          sort,
          offset,
          limit: PAGE_SIZE,
        });
        if (ticket !== requestRef.current) return;
        setBrowse({ page: result, signature });
        setError(null);
      } catch (e) {
        if (ticket !== requestRef.current) return;
        setPage(null);
        setError(String(e));
      } finally {
        if (ticket === requestRef.current) setSearching(false);
      }
    }, 300);
    return () => clearTimeout(debounceRef.current);
  }, [provider, kind, query, sort, filters, offset]);

  useEffect(() => {
    const node = scrollRef.current;
    if (!node) return;
    node.scrollTop = browse.scrollTop;
    return () => {
      setBrowse({ scrollTop: node.scrollTop });
    };
  }, []);

  const goToOffset = useCallback((next: number) => {
    setOffset(next);
    scrollRef.current?.scrollTo({ top: 0, behavior: "smooth" });
  }, []);

  const isCompatible = useCallback(
    (instance: Instance, project: ProjectSummary) => {
      const versionOk =
        project.game_versions.length === 0 ||
        project.game_versions.includes(instance.version_id);
      if (kind !== "mods") return versionOk;
      if (!instance.loader) return false;
      const accepted =
        instance.loader === "quilt" ? ["quilt", "fabric"] : [instance.loader];
      return versionOk && project.loaders.some((l) => accepted.includes(l));
    },
    [kind],
  );

  const beginInstall = async (project: ProjectSummary, into?: Instance) => {
    if (isPack && target?.isServer) {
      setInstallingPack(project.id);
      setError(null);
      setNotice(null);
      try {
        const versions = await api.listProjectVersions(provider, project.id, "modpacks", "", null);
        const newest = versions[0];
        if (!newest) throw new Error("This pack has no versions to install.");
        const created = await contentInstaller.installServerPack(provider, project.id, newest.id);
        if (!created) return;
        await refreshServers();
        setNotice(`Created server ${created.name}`);
      } catch (e) {
        setError(String(e));
      } finally {
        setInstallingPack(null);
      }
      return;
    }
    if (isPack) {
      setInstallingPack(project.id);
      setError(null);
      setNotice(null);
      try {
        const created = await contentInstaller.installLatestPack(
          provider,
          project.id,
          project.title,
          project.icon_url,
        );
        if (created) setNotice(`Created instance ${created.name}`);
      } catch (e) {
        setError(String(e));
      } finally {
        setInstallingPack(null);
      }
      return;
    }

    const destination: InstallTarget | null = into
      ? {
          id: into.id,
          name: into.name,
          version_id: into.version_id,
          loader: into.loader,
          isServer: false,
        }
      : target;
    if (!destination) {
      setNeedsTarget(project);
      return;
    }

    if (kind === "datapacks") {
      if (pinnedWorld) {
        setError(null);
        try {
          await api.installDatapack(provider, project.id, destination.id, pinnedWorld);
          setNotice(`Installed ${project.title} into ${pinnedWorld}`);
        } catch (e) {
          setError(String(e));
        }
        return;
      }
      setPlanning(project.id);
      try {
        const worlds = await api.listInstanceWorlds(destination.id);
        if (worlds.length === 0) {
          setError(`${destination.name} has no worlds yet, so a datapack has nowhere to go.`);
          return;
        }
        setWorldFilter("");
        setPickingWorld({ project, destination, worlds });
      } catch (e) {
        setError(String(e));
      } finally {
        setPlanning(null);
      }
      return;
    }

    setError(null);
    setNotice(null);
    try {
      const files = await contentInstaller.installContent({
        provider,
        projectId: project.id,
        instanceId: destination.isServer ? null : destination.id,
        serverId: destination.isServer ? destination.id : null,
        kind,
        gameVersion: destination.version_id,
        loader: kind === "mods" ? destination.loader : null,
        title: project.title,
        iconUrl: project.icon_url,
      });
      if (!files) return;
      setNotice(
        files.length > 1
          ? `Installed ${project.title} and ${files.length - 1} more into ${destination.name}`
          : `Installed ${project.title} into ${destination.name}`,
      );
    } catch (e) {
      setError(String(e));
    }
  };

  useEffect(() => {
    if (kind !== "datapacks" || !target) {
      setTargetWorlds([]);
      return;
    }
    let live = true;
    api
      .listInstanceWorlds(target.id)
      .then((worlds) => live && setTargetWorlds(worlds))
      .catch(() => live && setTargetWorlds([]));
    return () => {
      live = false;
    };
  }, [kind, target]);

  const matchingWorlds = useMemo(() => {
    const needle = worldFilter.trim().toLowerCase();
    const worlds = pickingWorld?.worlds ?? [];
    if (!needle) return worlds;
    return worlds.filter(
      (world) =>
        world.name.toLowerCase().includes(needle) ||
        world.folder_name.toLowerCase().includes(needle),
    );
  }, [pickingWorld, worldFilter]);

  const installIntoWorld = async (world: string) => {
    const request = pickingWorld;
    if (!request) return;
    setPickingWorld(null);
    setError(null);
    try {
      await api.installDatapack(provider, request.project.id, request.destination.id, world);
      setNotice(`Installed ${request.project.title} into ${world}`);
    } catch (e) {
      setError(String(e));
    }
  };

  const hits = page?.hits ?? [];
  const total = page?.total ?? 0;
  const activeFilters = countActive(filters);
  const pageIndex = Math.floor(offset / PAGE_SIZE);
  const lastPage = Math.max(0, Math.ceil(total / PAGE_SIZE) - 1);

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex items-center gap-3 border-b border-border-soft px-6 pt-5">
        <div className="flex gap-1">
          {KINDS.map((k) => {
            const disabled = k.id === "mods" && !!target && !target.loader;
            return (
              <button
                key={k.id}
                onClick={() => setKind(k.id)}
                disabled={disabled}
                title={disabled ? "Add a mod loader to this instance first" : undefined}
                className={cn(
                  "relative px-3.5 py-2.5 text-sm font-medium transition-colors",
                  kind === k.id
                    ? "text-content"
                    : "text-content-faint hover:text-content-muted",
                  disabled && "cursor-not-allowed opacity-35 hover:text-content-faint",
                )}
              >
                {k.label}
                {kind === k.id && (
                  <span className="absolute inset-x-2 -bottom-px h-0.5 rounded-full bg-(--accent) transition-colors duration-500" />
                )}
              </button>
            );
          })}
        </div>
        <div className="ml-auto flex items-center gap-2 pb-2">
          {kind === "datapacks" && target && (
            <WorldTargetPicker
              worlds={targetWorlds}
              selected={pinnedWorld}
              onSelect={setPinnedWorld}
            />
          )}
          {!isPack && (
            <InstanceTargetPicker
              instances={
                kind === "mods" ? instances.filter((instance) => !!instance.loader) : instances
              }
              selected={instances.find((instance) => instance.id === targetId) ?? null}
              onSelect={(instance) => setTarget(instance?.id ?? null)}
              servers={kind === "mods" ? contentServers : []}
              selectedServerId={serverId}
              onSelectServer={setDiscoverServer}
            />
          )}
        </div>
      </div>

      <div className="flex items-center gap-2 px-6 py-3">
        <button
          onClick={() => setShowFilters((v) => !v)}
          aria-label="Toggle filters"
          className={cn(
            "relative grid size-9 shrink-0 place-items-center rounded-lg border transition-colors",
            showFilters
              ? "border-border bg-surface-3 text-content"
              : "border-border bg-surface-2 text-content-faint hover:text-content",
          )}
        >
          <SlidersHorizontal className="size-4" />
          {activeFilters > 0 && (
            <span className="absolute -right-1 -top-1 grid size-4 place-items-center rounded-full bg-(--accent) text-[9px] font-bold text-black">
              {activeFilters}
            </span>
          )}
        </button>

        <div className="flex shrink-0 rounded-lg border border-border bg-surface-2 p-0.5">
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
            placeholder={`Search ${KINDS.find((k) => k.id === kind)?.label.toLowerCase()}`}
            autoFocus
            className="w-full rounded-lg border border-border bg-void py-2 pl-9 pr-3 text-sm text-content outline-none transition-colors focus:border-(--accent)"
          />
        </div>

        <div className="w-44 shrink-0">
          <Select
            value={SORTS.find((s) => s.id === sort)?.label ?? "Relevance"}
            options={SORTS.map((s) => s.label)}
            onChange={(label) =>
              setSort(SORTS.find((s) => s.label === label)?.id ?? "relevance")
            }
          />
        </div>

        <ResultViewToggle view={resultView} onChange={setResultView} />
      </div>

      {error && (
        <div className="mx-6 mb-2 flex items-start gap-2 rounded-lg border border-warn/30 bg-warn/10 px-3 py-2 text-xs text-warn">
          <TriangleAlert className="mt-0.5 size-3.5 shrink-0" />
          <span className="wrap-break-word">{error}</span>
        </div>
      )}
      {notice && (
        <div className="mx-6 mb-2 rounded-lg border border-ok/30 bg-ok/10 px-3 py-2 text-xs text-ok">
          {notice}
        </div>
      )}

      <div className="flex min-h-0 flex-1">
        {showFilters && (
          <FilterRail
            taxonomy={taxonomy}
            filters={filters}
            onChange={setFilters}
            showLoaders={usesLoaders}
            showEnvironment={provider === "modrinth" && kind === "mods"}
          />
        )}

        <div ref={scrollRef} className="min-h-0 flex-1 overflow-y-auto px-4 pb-6">
          {modsBlocked ? (
            <div className="flex flex-col items-center gap-3 py-20 text-center">
              <div className="grid size-12 place-items-center rounded-2xl border border-warn/30 bg-warn/10 text-warn">
                <TriangleAlert className="size-6" />
              </div>
              <div className="text-sm font-medium text-content">
                Mods are unavailable for vanilla instances
              </div>
              <p className="max-w-sm text-xs text-content-faint">
                Add Fabric, Forge, NeoForge, or Quilt to {target?.name ?? "this instance"} before
                browsing or installing mods.
              </p>
            </div>
          ) : searching && hits.length === 0 ? (
            <div className="flex items-center justify-center gap-2 py-16 text-sm text-content-muted">
              <Loader2 className="size-4 animate-spin" />
              Searching
            </div>
          ) : hits.length === 0 ? (
            <div className="flex flex-col items-center gap-2 py-16 text-center text-sm text-content-faint">
              <Package className="size-6" />
              No results
              {activeFilters > 0 && (
                <button
                  onClick={() => setFilters(emptyFilters)}
                  className="text-xs font-medium text-(--accent) hover:underline"
                >
                  Clear {activeFilters} {activeFilters === 1 ? "filter" : "filters"}
                </button>
              )}
            </div>
          ) : (
            <>
              <div
                className={cn(
                  "flex items-center justify-between px-2 py-2 text-xs text-content-muted",
                  searching && "opacity-60",
                )}
              >
                <span className="flex items-center gap-2">
                  <span>
                    <span className="font-medium text-content">{total.toLocaleString()}</span>{" "}
                    {total === 1 ? "result" : "results"}
                  </span>
                  {total > PAGE_SIZE && (
                    <span>
                      · page{" "}
                      <span className="font-medium tabular-nums text-content">
                        {pageIndex + 1}
                      </span>{" "}
                      of <span className="tabular-nums">{lastPage + 1}</span>
                    </span>
                  )}
                  {searching && <Loader2 className="size-3 animate-spin" />}
                </span>

                {total > PAGE_SIZE && (
                  <div className="flex items-center gap-1">
                    <button
                      onClick={() => goToOffset(0)}
                      disabled={pageIndex === 0}
                      title="First page"
                      aria-label="First page"
                      className="grid size-7 place-items-center rounded-md text-content-muted transition-colors hover:bg-surface-3 hover:text-content disabled:opacity-30 disabled:hover:bg-transparent"
                    >
                      <ChevronsLeft className="size-4" />
                    </button>
                    <button
                      onClick={() => goToOffset(Math.max(0, offset - PAGE_SIZE))}
                      disabled={pageIndex === 0}
                      title="Previous page"
                      aria-label="Previous page"
                      className="grid size-7 place-items-center rounded-md text-content-muted transition-colors hover:bg-surface-3 hover:text-content disabled:opacity-30 disabled:hover:bg-transparent"
                    >
                      <ChevronLeft className="size-4" />
                    </button>
                    <button
                      onClick={() => goToOffset(offset + PAGE_SIZE)}
                      disabled={pageIndex >= lastPage}
                      title="Next page"
                      aria-label="Next page"
                      className="grid size-7 place-items-center rounded-md text-content-muted transition-colors hover:bg-surface-3 hover:text-content disabled:opacity-30 disabled:hover:bg-transparent"
                    >
                      <ChevronRight className="size-4" />
                    </button>
                    <button
                      onClick={() => goToOffset(lastPage * PAGE_SIZE)}
                      disabled={pageIndex >= lastPage}
                      title="Last page"
                      aria-label="Last page"
                      className="grid size-7 place-items-center rounded-md text-content-muted transition-colors hover:bg-surface-3 hover:text-content disabled:opacity-30 disabled:hover:bg-transparent"
                    >
                      <ChevronsRight className="size-4" />
                    </button>
                  </div>
                )}
              </div>

              <ContentResults
                view={resultView}
                rows={hits.map((project) => {
                  const packInstance = instances.find(
                    (i) => i.pack_project_id === project.id,
                  );
                  const installedFile = sources?.[project.id]?.file_name;
                  const alsoIn = target ? [] : installedIn(project.id);
                  const busy =
                    planning === project.id ||
                    installingPack === project.id ||
                    activeProjects.has(project.id);
                  const done = !busy && !isPack && !!installedFile;
                  const liveTask = activeTasks.get(project.id);

                  return {
                    project,
                    subline: busy
                      ? liveTask
                        ? `${liveTask.stage}${
                            liveTask.total > 0
                              ? ` · ${liveTask.completed}/${liveTask.total}`
                              : ""
                          }`
                        : "Preparing"
                      : isPack
                        ? packInstance
                          ? `Installed as ${packInstance.name}`
                          : undefined
                        : installedFile
                          ? `Installed · ${installedFile}`
                          : alsoIn.length > 0
                            ? `Installed in ${alsoIn.map((i) => i.name).join(", ")}`
                            : undefined,
                    onOpen: () => openProject(provider, project.id, kind, project.title),
                    action: done ? (
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          if (isPack && packInstance) openInstance(packInstance.id);
                          else if (target?.isServer) openServer(target.id);
                          else if (target) openInstance(target.id);
                        }}
                        className="inline-flex h-8 shrink-0 items-center gap-1.5 rounded-lg bg-ok/15 px-3 text-xs font-semibold text-ok transition-colors hover:bg-ok/25"
                      >
                        <Check className="size-3.5" />
                        Installed
                      </button>
                    ) : (
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          void beginInstall(project);
                        }}
                        disabled={busy}
                        className="inline-flex h-8 shrink-0 items-center gap-1.5 rounded-lg px-3 text-xs font-semibold text-black shadow-md shadow-(color:--accent-glow) transition-all [background:linear-gradient(to_bottom,var(--accent),var(--accent-deep))] hover:[background:linear-gradient(to_bottom,var(--accent-bright),var(--accent))] disabled:opacity-60"
                      >
                        {busy ? (
                          <>
                            <Loader2 className="size-3.5 animate-spin" />
                            {liveTask && taskFraction(liveTask) != null
                              ? `${Math.round((taskFraction(liveTask) ?? 0) * 100)}%`
                              : "Installing"}
                          </>
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

              {total > PAGE_SIZE && (
                <div className="mt-4 flex items-center justify-center gap-2">
                  <button
                    onClick={() => goToOffset(Math.max(0, offset - PAGE_SIZE))}
                    disabled={pageIndex === 0}
                    className="inline-flex items-center gap-1 rounded-lg border border-border bg-surface-2 px-3 py-2 text-xs font-medium text-content-muted transition-colors hover:bg-surface-3 hover:text-content disabled:opacity-30"
                  >
                    <ChevronLeft className="size-3.5" />
                    Previous
                  </button>
                  <span className="px-2 text-xs tabular-nums text-content-faint">
                    {pageIndex + 1} / {lastPage + 1}
                  </span>
                  <button
                    onClick={() => goToOffset(offset + PAGE_SIZE)}
                    disabled={pageIndex >= lastPage}
                    className="inline-flex items-center gap-1 rounded-lg border border-border bg-surface-2 px-3 py-2 text-xs font-medium text-content-muted transition-colors hover:bg-surface-3 hover:text-content disabled:opacity-30"
                  >
                    Next
                    <ChevronRight className="size-3.5" />
                  </button>
                </div>
              )}
            </>
          )}
        </div>
      </div>

      {needsTarget && (
        <InstanceTargetPicker
          instances={instances}
          selected={null}
          modalFor={needsTarget.title}
          isCompatible={(instance) => isCompatible(instance, needsTarget)}
          isInstalled={(instance) =>
            !!allSources[`${instance.id}:${kind}`]?.[needsTarget.id]
          }
          onCancel={() => setNeedsTarget(null)}
          onSelect={(instance) => {
            const project = needsTarget;
            setNeedsTarget(null);
            if (instance) void beginInstall(project, instance);
          }}
        />
      )}
      {pickingWorld && (
        <Modal
          open
          onClose={() => setPickingWorld(null)}
          size="md"
          className="h-[min(560px,calc(100vh-48px))]"
          labelledBy="datapack-world-title"
        >
          <ModalHeader
            id="datapack-world-title"
            title="Which world?"
            subtitle={`${pickingWorld.project.title} goes into one world, not the whole instance`}
            onClose={() => setPickingWorld(null)}
          />
          {pickingWorld.worlds.length > 6 && (
            <div className="shrink-0 border-b border-border-soft px-5 py-3">
              <div className="relative">
                <Search className="pointer-events-none absolute left-3 top-1/2 size-3.5 -translate-y-1/2 text-content-faint" />
                <input
                  autoFocus
                  value={worldFilter}
                  onChange={(event) => setWorldFilter(event.target.value)}
                  placeholder="Filter worlds"
                  className="h-9 w-full rounded-lg border border-border bg-void pl-9 pr-3 text-sm text-content outline-none transition-colors placeholder:text-content-faint focus:border-(--accent)"
                />
              </div>
            </div>
          )}
          <div className="flex min-h-0 flex-1 flex-col gap-2 overflow-y-auto px-5 py-4">
            {matchingWorlds.length === 0 && (
              <p className="py-6 text-sm text-content-faint">Nothing matches that.</p>
            )}
            {matchingWorlds.map((world) => (
              <button
                key={world.folder_name}
                onClick={() => void installIntoWorld(world.folder_name)}
                className="flex items-center gap-3 rounded-xl border border-border-soft bg-surface-2/50 px-3.5 py-3 text-left transition-colors hover:border-border hover:bg-surface-2"
              >
                <span className="min-w-0 flex-1">
                  <span className="block truncate text-sm font-medium text-content">
                    {world.name}
                  </span>
                  <span className="block truncate font-mono text-[11px] text-content-faint">
                    {world.folder_name}
                  </span>
                </span>
              </button>
            ))}
          </div>
        </Modal>
      )}
    </div>
  );
}
