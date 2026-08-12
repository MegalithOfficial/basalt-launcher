import {
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  Check,
  Download,
  FileBox,
  ExternalLink,
  Loader2,
  Package,
  RotateCcw,
  TriangleAlert,
} from "lucide-react";
import { toast } from "sonner";

import { api } from "../lib/api";
import { cn } from "../lib/cn";
import { formatBytes } from "../lib/format";
import type {
  InstallPlan,
  ManualDownload,
  ManualDownloadSource,
  SearchProvider,
} from "../lib/types";
import {
  ContentInstallerContext,
  type ContentInstallOptions,
} from "../lib/contentInstaller";
import { useStore } from "../store";
import { InstallPlanPrompt } from "./InstallPlanPrompt";
import { Modal, ModalBody, ModalFooter, ModalHeader } from "./Modal";

interface InstallRequest {
  provider: SearchProvider;
  projectId: string;
  versionId: string;
  downloads: ManualDownload[];
  resolve: (sources: ManualDownloadSource[] | null) => void;
  reject: (error: unknown) => void;
}

interface BrowserDownloadRequest {
  downloads: ManualDownload[];
  resolve: (sources: ManualDownloadSource[] | null) => void;
}

interface ContentPlanRequest {
  plan: InstallPlan;
  resolve: (withDependencies: boolean | null) => void;
}

interface DownloadState {
  status: "waiting" | "ready" | "error";
  startedAt: number;
  path?: string;
  error?: string;
}

function key(download: ManualDownload) {
  return `${download.project_id}:${download.file_id}`;
}

function Row({
  download,
  state,
  active,
  index,
  onStart,
  innerRef,
}: {
  download: ManualDownload;
  state: DownloadState | undefined;
  active: boolean;
  index: number;
  onStart: () => void;
  innerRef?: React.Ref<HTMLDivElement>;
}) {
  const done = state?.status === "ready";
  const waiting = state?.status === "waiting";
  const failed = state?.status === "error";

  return (
    <div
      ref={innerRef}
      className={cn(
        "flex items-center gap-3 rounded-xl border px-3.5 py-3 transition-colors",
        failed
          ? "border-danger/30 bg-danger/[0.06]"
          : active
            ? "border-(--accent)/40 bg-(--accent)/[0.06]"
            : done
              ? "border-border-soft bg-surface-2/40"
              : "border-border-soft bg-surface-2/40 opacity-55",
      )}
    >
      <span
        className={cn(
          "grid size-8 shrink-0 place-items-center rounded-lg",
          done
            ? "bg-ok/15 text-ok"
            : failed
              ? "bg-danger/15 text-danger"
              : active
                ? "bg-(--accent)/15 text-(--accent)"
                : "bg-surface-3 text-content-faint",
        )}
      >
        {done ? (
          <Check className="size-4" strokeWidth={3} />
        ) : waiting ? (
          <Loader2 className="size-4 animate-spin" />
        ) : failed ? (
          <TriangleAlert className="size-4" />
        ) : download.pack_archive ? (
          <Package className="size-4" />
        ) : (
          <FileBox className="size-4" />
        )}
      </span>

      <span className="min-w-0 flex-1">
        <span className="flex min-w-0 items-center gap-2">
          <span className="truncate text-sm font-medium text-content">{download.file_name}</span>
          {download.pack_archive && (
            <span className="shrink-0 rounded bg-surface-3 px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-wide text-content-faint">
              pack
            </span>
          )}
        </span>
        <span className="mt-0.5 block truncate text-[11px] text-content-faint">
          {failed
            ? state?.error
            : done
              ? "verified and ready"
              : waiting
                ? "watching your Downloads folder"
                : active
                  ? "next up"
                  : `queued, number ${index + 1}`}
          {download.size != null && !failed && ` · ${formatBytes(download.size)}`}
        </span>
      </span>

      {(active || failed) && (
        <button
          type="button"
          onClick={onStart}
          disabled={waiting}
          className={cn(
            "inline-flex h-8 shrink-0 items-center gap-1.5 rounded-lg px-3 text-xs font-semibold transition-all",
            waiting
              ? "cursor-wait border border-border bg-surface-2 text-content-muted"
              : "text-black shadow-md shadow-(color:--accent-glow) [background:linear-gradient(to_bottom,var(--accent),var(--accent-deep))] hover:[background:linear-gradient(to_bottom,var(--accent-bright),var(--accent))]",
          )}
        >
          {waiting ? (
            <>
              <Loader2 className="size-3.5 animate-spin" />
              Waiting
            </>
          ) : failed ? (
            <>
              <RotateCcw className="size-3.5" />
              Try again
            </>
          ) : (
            <>
              <Download className="size-3.5" />
              Open CurseForge
            </>
          )}
        </button>
      )}
    </div>
  );
}

function ManualDownloadDialog({
  request,
  onClose,
  onReady,
  onError,
  resolveMore,
}: {
  request: { downloads: ManualDownload[] };
  onClose: () => void;
  onReady: (sources: ManualDownloadSource[]) => void;
  onError: (error: string) => void;
  resolveMore?: (sources: ManualDownloadSource[]) => Promise<ManualDownload[]>;
}) {
  const [requirements, setRequirements] = useState(request.downloads);
  const [downloads, setDownloads] = useState<Record<string, DownloadState>>({});
  const [resolving, setResolving] = useState(false);
  const polling = useRef(false);
  const advancing = useRef(false);
  const autoStarted = useRef<string | null>(null);
  const activeRow = useRef<HTMLDivElement>(null);
  const current = requirements.find((download) => downloads[key(download)]?.status !== "ready");
  const readyCount = requirements.filter(
    (download) => downloads[key(download)]?.status === "ready",
  ).length;

  useEffect(() => {
    const interval = window.setInterval(async () => {
      if (polling.current) return;
      if (!current || downloads[key(current)]?.status !== "waiting") return;
      polling.current = true;
      try {
        const state = downloads[key(current)];
        const path = await api.findCurseforgeDownload(current, state.startedAt);
        if (path) {
          setDownloads((downloads) => ({
            ...downloads,
            [key(current)]: { ...state, status: "ready", path },
          }));
        }
      } catch (error) {
        const state = downloads[key(current)];
        setDownloads((downloads) => ({
          ...downloads,
          [key(current)]: { ...state, status: "error", error: String(error) },
        }));
      } finally {
        polling.current = false;
      }
    }, 1_000);
    return () => window.clearInterval(interval);
  }, [current, downloads]);

  useEffect(() => {
    if (current || requirements.length === 0 || advancing.current) return;
    advancing.current = true;
    setResolving(true);
    const sources: ManualDownloadSource[] = requirements.map((download) => ({
      project_id: download.project_id,
      file_id: download.file_id,
      path: downloads[key(download)]?.path ?? "",
    }));
    void Promise.resolve(resolveMore?.(sources) ?? [])
      .then((next) => {
        const additions = next.filter(
          (download) => !requirements.some((existing) => key(existing) === key(download)),
        );
        if (additions.length > 0) {
          setRequirements((requirements) => [...requirements, ...additions]);
          advancing.current = false;
          setResolving(false);
          return;
        }
        onReady(sources);
      })
      .catch((error) => {
        onError(String(error));
        onClose();
      });
  }, [current, downloads, onClose, onError, onReady, requirements, resolveMore]);

  const currentKey = current ? key(current) : null;

  useEffect(() => {
    if (!currentKey) return;
    activeRow.current?.scrollIntoView({ block: "center", behavior: "smooth" });
  }, [currentKey]);

  const begin = useCallback(async (download: ManualDownload) => {
    const startedAt = Date.now();
    setDownloads((current) => ({
      ...current,
      [key(download)]: { status: "waiting", startedAt },
    }));
    try {
      await openUrl(download.download_page_url);
    } catch (error) {
      setDownloads((current) => ({
        ...current,
        [key(download)]: { status: "error", startedAt, error: String(error) },
      }));
    }
  }, []);

  useEffect(() => {
    if (resolving || !current || readyCount === 0 || downloads[key(current)]) return;
    const currentKey = key(current);
    if (autoStarted.current === currentKey) return;
    autoStarted.current = currentKey;
    void begin(current);
  }, [begin, current, downloads, readyCount, resolving]);

  const total = requirements.length;
  const fraction = total === 0 ? 0 : readyCount / total;

  return (
    <Modal
      open
      onClose={onClose}
      size="wide"
      nested
      className="h-[min(620px,calc(100vh-48px))]"
      dismissable={!resolving}
      labelledBy="curseforge-download-title"
    >
      <ModalHeader
        id="curseforge-download-title"
        title="A few files need a manual download"
        subtitle="CurseForge lets authors block automatic downloads. Fetch these in your browser and Basalt takes it from there."
        icon={
          <div className="grid size-9 place-items-center rounded-xl border border-warn/25 bg-warn/10 text-warn">
            <ExternalLink className="size-4" />
          </div>
        }
        onClose={resolving ? undefined : onClose}
      />

      <div className="shrink-0 border-b border-border-soft px-5 py-3.5">
        <div className="flex items-baseline justify-between gap-4">
          <span className="text-xs font-medium text-content">
            {readyCount === total && total > 0
              ? "All files verified"
              : `${readyCount} of ${total} verified`}
          </span>
          <span className="text-[11px] text-content-faint">
            Basalt watches Downloads and checks the name, size and checksum
          </span>
        </div>
        <div className="mt-2 h-1 overflow-hidden rounded-full bg-surface-3">
          <div
            className="h-full rounded-full bg-(--accent) transition-[width] duration-300"
            style={{ width: `${Math.round(fraction * 100)}%` }}
          />
        </div>
      </div>

      <ModalBody className="flex flex-col gap-2">
        {requirements.map((download, index) => (
          <Row
            key={key(download)}
            download={download}
            state={downloads[key(download)]}
            active={!!current && key(current) === key(download) && !resolving}
            index={index}
            onStart={() => void begin(download)}
            innerRef={current && key(current) === key(download) ? activeRow : undefined}
          />
        ))}

        {resolving && (
          <div className="flex items-center gap-3 rounded-xl border border-border-soft bg-surface-2/60 px-3.5 py-3 text-xs text-content-muted">
            <Loader2 className="size-4 shrink-0 animate-spin text-(--accent)" />
            Checking the pack for more restricted files
          </div>
        )}
      </ModalBody>

      <ModalFooter className="justify-between">
        <span className="text-[11px] text-content-faint">
          Closing this leaves anything you already downloaded where it is.
        </span>
        <button
          type="button"
          onClick={onClose}
          disabled={resolving}
          className="h-9 rounded-lg px-3.5 text-xs font-medium text-content-muted transition-colors hover:bg-surface-3 hover:text-content disabled:opacity-50"
        >
          Cancel
        </button>
      </ModalFooter>
    </Modal>
  );
}

export function ContentInstallerProvider({ children }: { children: React.ReactNode }) {
  const installModpack = useStore((state) => state.installModpack);
  const installContentShared = useStore((state) => state.installContent);
  const beginOptimisticTask = useStore((state) => state.beginOptimisticTask);
  const endOptimisticTask = useStore((state) => state.endOptimisticTask);
  const [request, setRequest] = useState<InstallRequest | null>(null);
  const [contentRequest, setContentRequest] = useState<ContentPlanRequest | null>(null);
  const [installingVersionId, setInstallingVersionId] = useState<string | null>(null);

  const collectSources = useCallback(
    async (
      provider: SearchProvider,
      projectId: string,
      versionId: string,
    ): Promise<ManualDownloadSource[] | null> => {
      let sources: ManualDownloadSource[] = [];
      if (provider === "curseforge") {
        const plan = await api.planModpackInstall(provider, projectId, versionId);
        if (plan.manual_downloads.length > 0) {
          const collected = await new Promise<ManualDownloadSource[] | null>(
            (resolve, reject) => {
              setRequest({
                provider,
                projectId,
                versionId,
                downloads: plan.manual_downloads,
                resolve,
                reject,
              });
            },
          );
          if (!collected) return null;
          sources = collected;
          toast.info("Browser downloads verified", {
            description: "Installing the pack.",
          });
        }
      }
      return sources;
    },
    [],
  );

  const runInstall = useCallback(
    async (provider: SearchProvider, projectId: string, versionId: string) => {
      const sources = await collectSources(provider, projectId, versionId);
      if (!sources) return null;
      return await installModpack(provider, projectId, versionId, sources);
    },
    [collectSources, installModpack],
  );

  const installServerPack = useCallback(
    async (provider: SearchProvider, projectId: string, versionId: string) => {
      const sources = await collectSources(provider, projectId, versionId);
      if (!sources) return null;
      return await api.installServerPack(provider, projectId, versionId, sources);
    },
    [collectSources],
  );

  const installPack = useCallback(
    async (
      provider: SearchProvider,
      projectId: string,
      versionId: string,
      title = "Modpack",
      iconUrl: string | null = null,
    ) => {
      setInstallingVersionId(versionId);
      const taskId = beginOptimisticTask("modpack_install", title, {
        subtitle: "Preparing the pack",
        iconUrl,
        projectId,
      });
      try {
        return await runInstall(provider, projectId, versionId);
      } catch (error) {
        toast.error(`Could not install ${title}`, { description: String(error) });
        throw error;
      } finally {
        endOptimisticTask(taskId);
        setInstallingVersionId(null);
      }
    },
    [beginOptimisticTask, endOptimisticTask, runInstall],
  );

  const installLatestPack = useCallback(
    async (
      provider: SearchProvider,
      projectId: string,
      title = "Modpack",
      iconUrl: string | null = null,
    ) => {
      const taskId = beginOptimisticTask("modpack_install", title, {
        subtitle: "Finding a compatible version",
        iconUrl,
        projectId,
      });
      try {
        const versions = await api.listProjectVersions(
          provider,
          projectId,
          "modpacks",
          "",
          null,
        );
        const preferred =
          versions.find((version) => version.channel === "release") ?? versions[0];
        if (!preferred) {
          throw new Error("This pack has no installable versions.");
        }
        setInstallingVersionId(preferred.id);
        return await runInstall(provider, projectId, preferred.id);
      } catch (error) {
        toast.error(`Could not install ${title}`, { description: String(error) });
        throw error;
      } finally {
        endOptimisticTask(taskId);
        setInstallingVersionId(null);
      }
    },
    [beginOptimisticTask, endOptimisticTask, runInstall],
  );

  const installContent = useCallback(
    async (options: ContentInstallOptions) => {
      const taskId = beginOptimisticTask(
        "content_install",
        options.title ?? "Content",
        {
          subtitle: "Planning install",
          iconUrl: options.iconUrl,
          instanceId: options.instanceId,
          projectId: options.projectId,
        },
      );
      try {
        const plan = options.serverId
          ? await api.planServerContentInstall(
              options.serverId,
              options.provider,
              options.projectId,
              options.versionId ?? null,
            )
          : await api.planContentInstall(
              options.provider,
              options.projectId,
              options.instanceId ?? "",
              options.kind,
              options.gameVersion,
              options.loader,
              options.versionId ?? null,
            );
        const replaces =
          !!plan.primary?.replaces || plan.dependencies.some((file) => !!file.replaces);
        const trivial =
          plan.dependencies.length === 0 &&
          plan.skipped.length === 0 &&
          plan.conflicts.length === 0 &&
          !replaces;
        let withDependencies = true;
        if (!trivial) {
          const choice = await new Promise<boolean | null>((resolve) => {
            setContentRequest({ plan, resolve });
          });
          if (choice === null) return null;
          withDependencies = choice;
        }
        return await installContentShared({
          provider: options.provider,
          projectId: options.projectId,
          instanceId: options.instanceId,
          serverId: options.serverId,
          kind: options.kind,
          gameVersion: options.gameVersion,
          loader: options.loader,
          versionId: options.versionId,
          withDependencies,
        });
      } catch (error) {
        toast.error(`Could not install ${options.title ?? "content"}`, {
          description: String(error),
        });
        throw error;
      } finally {
        endOptimisticTask(taskId);
      }
    },
    [beginOptimisticTask, endOptimisticTask, installContentShared],
  );

  const finishContentRequest = useCallback(
    (withDependencies: boolean | null) => {
      contentRequest?.resolve(withDependencies);
      setContentRequest(null);
    },
    [contentRequest],
  );

  const finishRequest = useCallback(
    (sources: ManualDownloadSource[] | null) => {
      request?.resolve(sources);
      setRequest(null);
    },
    [request],
  );

  const failRequest = useCallback(
    (error: string) => {
      request?.reject(new Error(error));
      setRequest(null);
    },
    [request],
  );

  const value = useMemo(
    () => ({
      installContent,
      installPack,
      installLatestPack,
      installServerPack,
      installingVersionId,
    }),
    [installContent, installPack, installLatestPack, installServerPack, installingVersionId],
  );

  return (
    <ContentInstallerContext.Provider value={value}>
      {children}
      <InstallPlanPrompt
        plan={contentRequest?.plan ?? null}
        busy={false}
        progress={null}
        onConfirm={() => finishContentRequest(true)}
        onSkipDependencies={() => finishContentRequest(false)}
        onCancel={() => finishContentRequest(null)}
      />
      {request && (
        <ManualDownloadDialog
          request={request}
          resolveMore={(sources) =>
            api
              .planModpackInstall(request.provider, request.projectId, request.versionId, sources)
              .then((plan) => plan.manual_downloads)
          }
          onClose={() => finishRequest(null)}
          onReady={finishRequest}
          onError={failRequest}
        />
      )}
    </ContentInstallerContext.Provider>
  );
}

export function useContentInstaller() {
  return useContext(ContentInstallerContext);
}

export function useCurseforgeDownloads() {
  const [request, setRequest] = useState<BrowserDownloadRequest | null>(null);

  const collect = useCallback(
    (downloads: ManualDownload[]) =>
      new Promise<ManualDownloadSource[] | null>((resolve) => {
        setRequest({ downloads, resolve });
      }),
    [],
  );

  const finish = useCallback(
    (sources: ManualDownloadSource[] | null) => {
      request?.resolve(sources);
      setRequest(null);
    },
    [request],
  );

  return {
    collect,
    modal: request ? (
      <ManualDownloadDialog
        request={request}
        onClose={() => finish(null)}
        onReady={(sources) => finish(sources)}
        onError={(error) => {
          toast.error("Could not verify the browser download", { description: error });
        }}
      />
    ) : null,
  };
}
