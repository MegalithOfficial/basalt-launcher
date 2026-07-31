import { useEffect, useState } from "react";
import { Loader2, Package, TriangleAlert } from "lucide-react";

import { cn } from "../lib/cn";
import { api } from "../lib/api";
import type {
  Changelog,
  ContentKind,
  InstallPlan,
  ProjectDetails,
  ProjectSummary,
  ProjectVersion,
} from "../lib/types";
import { InstallPlanPrompt } from "../components/InstallPlanPrompt";
import { useModpackInstaller } from "../components/CurseForgeDownloadModal";
import { InstanceTargetPicker } from "../components/InstanceTargetPicker";
import { Markdown } from "../components/project/Markdown";
import { ProjectGallery } from "../components/project/ProjectGallery";
import { ProjectHero } from "../components/project/ProjectHero";
import { ProjectSidebar } from "../components/project/ProjectSidebar";
import { VersionBrowser } from "../components/project/VersionBrowser";
import { useActiveProjectIds, useInstanceTask } from "../lib/useTasks";
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
  const contentProgress = useInstanceTask(instance?.id);
  const activeProjects = useActiveProjectIds();
  const openProject = useStore((s) => s.openProject);
  const openInstance = useStore((s) => s.openInstance);
  const packInstance = useStore((s) =>
    s.searchKind === "modpacks" && s.projectRef
      ? (s.instances.find((i) => i.pack_project_id === s.projectRef?.id) ?? null)
      : null,
  );
  const sourcesMap = useStore((s) => s.contentSources[`${instance?.id}:${s.searchKind}`]);
  const refreshContentSources = useStore((s) => s.refreshContentSources);
  const installContentShared = useStore((s) => s.installContent);

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
  const [pending, setPending] = useState<{ target: PendingInstall; plan: InstallPlan } | null>(
    null,
  );
  const [needsTarget, setNeedsTarget] = useState<PendingInstall | null>(null);
  const [pickingTarget, setPickingTarget] = useState(false);

  const isPack = kind === "modpacks";
  const loader = kind === "mods" ? (instance?.loader ?? null) : null;
  const modpackInstaller = useModpackInstaller({
    onInstalled: (created) => setNotice(`Created instance ${created.name}`),
    onError: setError,
  });

  useEffect(() => {
    if (instance && storeKind) void refreshContentSources(instance.id, storeKind);
  }, [instance?.id, storeKind, refreshContentSources]);

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
  }, [instance?.id]);

  useEffect(() => {
    if (tab !== "versions" || versions !== null || !projectRef) return;
    let live = true;
    api
      .listProjectVersions(
        projectRef.provider,
        projectRef.id,
        kind,
        instance?.version_id ?? "",
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
  }, [tab, versions, projectRef?.id, instance?.id, kind, loader]);

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
        const list =
          versions ??
          (await api.listProjectVersions(projectRef.provider, projectRef.id, "modpacks", "", null));
        const preferred = list.find((v) => v.channel === "release") ?? list[0];
        if (!preferred) {
          setError("This pack has no installable versions.");
          return;
        }
        vid = preferred.id;
      }
      const created = await modpackInstaller.install(
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

  const doInstall = async (target: PendingInstall, withDependencies: boolean) => {
    if (!instance) return;
    setInstalling(target.key);
    setError(null);
    setNotice(null);
    try {
      const files = await installContentShared({
        provider: projectRef.provider,
        projectId: target.projectId,
        instanceId: instance.id,
        kind,
        gameVersion: instance.version_id,
        loader,
        versionId: target.versionId,
        withDependencies,
      });
      setPending(null);
      setInstalled((prev) => new Set(prev).add(target.key));
      setNotice(
        files.length > 1
          ? `Installed ${files[0]?.title ?? "the file"} and ${files.length - 1} more into ${instance.name}`
          : `Installed ${files[0]?.title ?? "the file"} into ${instance.name}`,
      );
    } catch (e) {
      setPending(null);
      setError(String(e));
    } finally {
      setInstalling(null);
    }
  };

  const beginInstall = async (target: PendingInstall) => {
    if (isPack) {
      await installPack(target.versionId);
      return;
    }
    if (!instance) {
      setNeedsTarget(target);
      return;
    }
    setInstalling(target.key);
    setError(null);
    try {
      const plan = await api.planContentInstall(
        projectRef.provider,
        target.projectId,
        instance.id,
        kind,
        instance.version_id,
        loader,
        target.versionId,
      );
      const replaces =
        !!plan.primary?.replaces || plan.dependencies.some((file) => !!file.replaces);
      const trivial =
        plan.dependencies.length === 0 &&
        plan.skipped.length === 0 &&
        plan.conflicts.length === 0 &&
        !replaces;
      if (!trivial) {
        setInstalling(null);
        setPending({ target, plan });
        return;
      }
    } catch (e) {
      setInstalling(null);
      setError(String(e));
      return;
    }
    await doInstall(target, true);
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
        showTargetPicker={!isPack}
        onInstall={() => install(null)}
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
              instanceVersion={instance?.version_id ?? null}
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
                instanceVersion={instance?.version_id ?? null}
                instanceLoader={loader}
                hasInstance={!!instance}
                installedVersionId={installedEntry?.version_id ?? null}
                installingKey={installing ?? modpackInstaller.installingVersionId}
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

      <InstallPlanPrompt
        plan={pending?.plan ?? null}
        busy={installing !== null}
        progress={contentProgress ?? null}
        onConfirm={() => pending && doInstall(pending.target, true)}
        onSkipDependencies={() => pending && doInstall(pending.target, false)}
        onCancel={() => setPending(null)}
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
              if (target) setTimeout(() => void beginInstall(target), 0);
            }
          }}
        />
      )}
      {modpackInstaller.modal}
    </div>
  );
}
