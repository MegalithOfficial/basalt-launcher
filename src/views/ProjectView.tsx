import { useEffect, useMemo, useState } from "react";
import { Loader2, Package, TriangleAlert } from "lucide-react";

import { cn } from "../lib/cn";
import { api } from "../lib/api";
import type {
  Changelog,
  ContentKind,
  ProjectDetails,
  ProjectSummary,
  ProjectVersion,
  VersionFile,
} from "../lib/types";
import { useContentInstaller } from "../components/CurseForgeDownloadModal";
import { GetServerModal } from "../components/GetServerModal";
import { InstanceTargetPicker } from "../components/InstanceTargetPicker";
import { Markdown } from "../components/project/Markdown";
import { ProjectGallery } from "../components/project/ProjectGallery";
import { ProjectHero } from "../components/project/ProjectHero";
import { ProjectSidebar } from "../components/project/ProjectSidebar";
import { VersionBrowser } from "../components/project/VersionBrowser";
import { useActiveProjectIds } from "../lib/useTasks";
import { serverPackFile } from "../lib/servers";
import type { InstallTarget } from "../lib/target";
import { useStore } from "../store";

interface PendingInstall {
  key: string;
  projectId: string;
  versionId: string | null;
}

type Tab = "description" | "versions" | "gallery";

export function ProjectView() {
  const projectRef = useStore((s) => s.projectRef);
  const storeKind = useStore((s) => s.searchKind);
  const kind: ContentKind = storeKind ?? "mods";
  const instance = useStore((s) =>
    s.instances.find((i) => i.id === (s.detailInstanceId ?? s.discoverTargetId)),
  );
  const instances = useStore((s) => s.instances);
  const setDiscoverTarget = useStore((s) => s.setDiscoverTarget);
  const setDiscoverServer = useStore((s) => s.setDiscoverServer);
  const serverId = useStore((s) => s.discoverServerId);
  const servers = useStore((s) => s.servers);
  const serverSoftware = useStore((s) => s.serverSoftware);
  const refreshServerContentSources = useStore((s) => s.refreshServerContentSources);
  const server = servers.find((entry) => entry.id === serverId) ?? null;
  const contentServers = useMemo(
    () =>
      servers.filter(
        (entry) =>
          entry.available &&
          !!serverSoftware.find((spec) => spec.id === entry.flavor)?.content_dir,
      ),
    [servers, serverSoftware],
  );
  const destination: InstallTarget | null = server
    ? {
        id: server.id,
        name: server.name,
        version_id: server.version_id,
        loader: server.flavor,
        isServer: true,
      }
    : instance
      ? {
          id: instance.id,
          name: instance.name,
          version_id: instance.version_id,
          loader: instance.loader,
          isServer: false,
        }
      : null;
  const activeProjects = useActiveProjectIds();
  const openProject = useStore((s) => s.openProject);
  const openInstance = useStore((s) => s.openInstance);
  const packInstance = useStore((s) =>
    s.searchKind === "modpacks" && s.projectRef
      ? (s.instances.find((i) => i.pack_project_id === s.projectRef?.id) ?? null)
      : null,
  );
  const sourcesMap = useStore(
    (s) => s.contentSources[`${s.discoverServerId ?? instance?.id}:${s.searchKind}`],
  );
  const refreshContentSources = useStore((s) => s.refreshContentSources);

  const [tab, setTab] = useState<Tab>("description");
  const [details, setDetails] = useState<ProjectDetails | null>(null);
  const [versions, setVersions] = useState<ProjectVersion[] | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [installing, setInstalling] = useState<string | null>(null);
  const [installed, setInstalled] = useState<Set<string>>(new Set());
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [changelogs, setChangelogs] = useState<Record<string, Changelog | "loading">>({});
  const [resolvedProjects, setResolvedProjects] = useState<
    Record<string, ProjectSummary | null>
  >({});
  const [needsTarget, setNeedsTarget] = useState<PendingInstall | null>(null);
  const [serverPack, setServerPack] = useState<{
    version: ProjectVersion;
    file: VersionFile;
    fileId: string | null;
  } | null>(null);
  const [pickingTarget, setPickingTarget] = useState(false);

  const isPack = kind === "modpacks";
  const loader = kind === "mods" ? (destination?.loader ?? null) : null;
  const contentInstaller = useContentInstaller();

  useEffect(() => {
    if (serverId) void refreshServerContentSources(serverId);
    else if (instance && storeKind) void refreshContentSources(instance.id, storeKind);
  }, [instance?.id, serverId, storeKind, refreshContentSources, refreshServerContentSources]);

  useEffect(() => {
    if (!projectRef) return;
    let live = true;
    setLoading(true);
    setDetails(null);
    setVersions(null);
    setInstalled(new Set());
    setTab("description");
    setError(null);
    setNotice(null);
    setExpandedId(null);
    setChangelogs({});
    setResolvedProjects({});
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
    setVersions(null);
  }, [destination?.id, destination?.version_id, loader]);

  useEffect(() => {
    if (versions !== null || !projectRef) return;
    let live = true;
    api
      .listProjectVersions(
        projectRef.provider,
        projectRef.id,
        kind,
        destination?.version_id ?? "",
        loader,
      )
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
  }, [
    versions,
    projectRef?.provider,
    projectRef?.id,
    destination?.id,
    destination?.version_id,
    kind,
    loader,
  ]);

  if (!projectRef) {
    return (
      <div className="flex flex-1 flex-col items-center justify-center gap-2 text-sm text-content-muted">
        <Package className="size-6 text-content-faint" />
        No project selected.
      </div>
    );
  }

  const busyProject = installing !== null || activeProjects.has(projectRef.id);

  const installedEntry = busyProject
    ? null
    : isPack
      ? packInstance
        ? { file_name: packInstance.name, version_id: packInstance.pack_version_id }
        : null
      : (sourcesMap?.[projectRef.id] ?? null);

  const installPack = async (versionId: string | null) => {
    setInstalling(versionId ?? "latest");
    setError(null);
    setNotice(null);
    try {
      let vid = versionId;
      if (!vid) {
        const created = await contentInstaller.installLatestPack(
          projectRef.provider,
          projectRef.id,
          details?.title ?? projectRef.title ?? "Modpack",
          details?.icon_url ?? null,
        );
        if (created) setNotice(`Created instance ${created.name}`);
        return;
      }
      const created = await contentInstaller.installPack(
        projectRef.provider,
        projectRef.id,
        vid,
        details?.title ?? projectRef.title ?? "Modpack",
        details?.icon_url ?? null,
      );
      if (created) setNotice(`Created instance ${created.name}`);
    } catch (e) {
      setError(String(e));
    } finally {
      setInstalling(null);
    }
  };

  const doInstall = async (target: PendingInstall, into: InstallTarget) => {
    setInstalling(target.key);
    setError(null);
    setNotice(null);
    try {
      const files = await contentInstaller.installContent({
        provider: projectRef.provider,
        projectId: target.projectId,
        instanceId: into.isServer ? null : into.id,
        serverId: into.isServer ? into.id : null,
        kind,
        gameVersion: into.version_id,
        loader: kind === "mods" ? into.loader : null,
        versionId: target.versionId,
        title: details?.title ?? projectRef.title ?? "Content",
        iconUrl: details?.icon_url ?? null,
      });
      if (!files) return;
      setInstalled((prev) => new Set(prev).add(target.key));
      setNotice(
        files.length > 1
          ? `Installed ${files[0]?.title ?? "the file"} and ${files.length - 1} more into ${into.name}`
          : `Installed ${files[0]?.title ?? "the file"} into ${into.name}`,
      );
    } catch (e) {
      setError(String(e));
    } finally {
      setInstalling(null);
    }
  };

  const heroServerVersion = useMemo(
    () =>
      (versions ?? []).find(
        (version) => version.server_pack_file_id || serverPackFile(version.files),
      ) ?? null,
    [versions],
  );

  const openServerPack = async (version: ProjectVersion) => {
    setError(null);
    try {
      const local = serverPackFile(version.files);
      const file = local
        ? local
        : version.server_pack_file_id
          ? await api.getServerPackFile(projectRef.id, version.server_pack_file_id, version.id)
          : null;
      if (!file) {
        setError("This version does not publish a server pack.");
        return;
      }
      setServerPack({ version, file, fileId: version.server_pack_file_id });
    } catch (cause) {
      setError(String(cause));
    }
  };

  const beginInstall = async (
    target: PendingInstall,
    into: InstallTarget | null = destination,
  ) => {
    if (isPack) {
      await installPack(target.versionId);
      return;
    }
    if (!into) {
      setNeedsTarget(target);
      return;
    }
    await doInstall(target, into);
  };

  const install = (versionId: string | null) =>
    beginInstall({ key: versionId ?? "latest", projectId: projectRef.id, versionId });

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
        .then((results) =>
          setResolvedProjects((prev) => {
            const next = { ...prev };
            results.forEach((r) => (next[r.id] = r));
            return next;
          }),
        )
        .catch(() => {});
    }

    if (changelogs[v.id]) return;
    if (v.changelog) {
      setChangelogs((prev) => ({ ...prev, [v.id]: { body: v.changelog!, format: "markdown" } }));
      return;
    }
    setChangelogs((prev) => ({ ...prev, [v.id]: "loading" }));
    try {
      const changelog = await api.getVersionChangelog(projectRef.provider, projectRef.id, v.id);
      setChangelogs((prev) => ({ ...prev, [v.id]: changelog }));
    } catch {
      setChangelogs((prev) => ({ ...prev, [v.id]: { body: "", format: "markdown" } }));
    }
  };

  const gallery = details?.gallery ?? [];
  const tabs: Array<{ id: Tab; label: string }> = [
    { id: "description", label: "Description" },
    { id: "versions", label: "Versions" },
    ...(gallery.length > 0 ? [{ id: "gallery" as Tab, label: "Gallery" }] : []),
  ];

  return (
    <div className="-mt-9 flex min-h-0 flex-1 flex-col">
      <ProjectHero
        details={details}
        provider={projectRef.provider}
        loading={loading}
        isPack={isPack}
        installedLabel={installedEntry && !isPack ? "Installed" : null}
        installedNote={
          isPack && packInstance && !busyProject ? `Installed as ${packInstance.name}` : null
        }
        installing={busyProject}
        instances={instances}
        target={instance ?? null}
        onSelectTarget={(picked) => setDiscoverTarget(picked?.id ?? null)}
        servers={kind === "mods" ? contentServers : []}
        selectedServerId={serverId}
        onSelectServer={setDiscoverServer}
        showTargetPicker={!isPack}
        onInstall={() => install(null)}
        onGetServer={
          heroServerVersion ? () => void openServerPack(heroServerVersion) : undefined
        }
        onOpenInstalled={() =>
          isPack && packInstance ? openInstance(packInstance.id) : setTab("versions")
        }
      />

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
              <span className="absolute inset-x-2 -bottom-px h-0.5 rounded-full bg-(--accent) transition-colors duration-500" />
            )}
          </button>
        ))}
      </div>

      {error && (
        <div className="mx-6 mt-3 flex items-start gap-2 rounded-lg border border-warn/30 bg-warn/10 px-3 py-2 text-xs text-warn">
          <TriangleAlert className="mt-0.5 size-3.5 shrink-0" />
          <span className="wrap-break-word">{error}</span>
        </div>
      )}
      {notice && (
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
              {details.body.trim() ? (
                <Markdown body={details.body} format={details.body_format} />
              ) : (
                <p className="text-sm text-content-faint">
                  This project has no description.
                </p>
              )}
            </div>
            <ProjectSidebar
              details={details}
              instanceVersion={destination?.version_id ?? null}
              instanceLoader={loader}
            />
          </div>
        ) : tab === "versions" ? (
          <div className="mx-auto max-w-4xl px-6 py-4">
            {versions === null ? (
              <div className="flex items-center justify-center gap-2 py-12 text-sm text-content-muted">
                <Loader2 className="size-4 animate-spin" />
                Loading versions
              </div>
            ) : (
              <VersionBrowser
                versions={versions}
                kind={kind}
                isPack={isPack}
                instanceVersion={destination?.version_id ?? null}
                instanceLoader={loader}
                hasInstance={!!destination}
                installedVersionId={installedEntry?.version_id ?? null}
                onGetServer={(version) => void openServerPack(version)}
                installingKey={installing ?? contentInstaller.installingVersionId}
                installedKeys={installed}
                resolvedProjects={resolvedProjects}
                changelogs={changelogs}
                websiteUrl={details?.website_url ?? null}
                provider={projectRef.provider}
                expandedId={expandedId}
                onExpand={toggleExpand}
                onInstall={(versionId) => install(versionId)}
                onInstallDependency={(dep) =>
                  beginInstall({ key: `dep:${dep.id}`, projectId: dep.id, versionId: null })
                }
                onOpenProject={(projectId) =>
                  openProject(projectRef.provider, projectId, kind)
                }
                onChooseInstance={() => setPickingTarget(true)}
              />
            )}
          </div>
        ) : tab === "gallery" ? (
          <ProjectGallery images={gallery} />
        ) : null}
      </div>

      <GetServerModal
        open={serverPack !== null}
        title={details?.title ?? projectRef.title ?? "Modpack"}
        version={serverPack?.version ?? null}
        file={serverPack?.file ?? null}
        fileId={serverPack?.fileId ?? null}
        projectId={projectRef.id}
        onClose={() => setServerPack(null)}
      />

      {(needsTarget || pickingTarget) && (
        <InstanceTargetPicker
          instances={instances}
          selected={null}
          modalFor={details?.title ?? "this project"}
          onSelect={(picked) => {
            const target = needsTarget;
            setNeedsTarget(null);
            setPickingTarget(false);
            if (picked) {
              setDiscoverTarget(picked.id);
              if (target) {
                void beginInstall(target, {
                  id: picked.id,
                  name: picked.name,
                  version_id: picked.version_id,
                  loader: picked.loader,
                  isServer: false,
                });
              }
            }
          }}
        />
      )}
    </div>
  );
}
