import { useEffect, useState } from "react";
import {
  CircleStop,
  Download,
  FolderOpen,
  Loader2,
  Play,
  Server as ServerIcon,
  Zap,
} from "lucide-react";

import { ConfirmDialog } from "../components/ConfirmDialog";
import { ConsolePanel } from "../components/servers/ConsolePanel";
import { ServerStatusPill } from "../components/servers/ServerStatusPill";
import { UsageMeter } from "../components/servers/UsageMeter";
import { api } from "../lib/api";
import { openFolder } from "../lib/reveal";
import { flavorLabel, isLive, serverAddress } from "../lib/servers";
import { useUptime } from "../lib/useUptime";
import { EmptyState } from "../components/ui";
import { useStore } from "../store";

const EMPTY_USAGE: never[] = [];

export function ServerView() {
  const detailServerId = useStore((s) => s.detailServerId);
  const servers = useStore((s) => s.servers);
  const serverRunning = useStore((s) => s.serverRunning);
  const startServer = useStore((s) => s.startServer);
  const stopServer = useStore((s) => s.stopServer);
  const forceStopServer = useStore((s) => s.forceStopServer);
  const installServer = useStore((s) => s.installServer);

  const server = servers.find((entry) => entry.id === detailServerId);
  const usage = useStore((s) => (detailServerId ? s.serverUsage[detailServerId] : undefined));
  const info = detailServerId ? serverRunning[detailServerId] : undefined;

  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [forcing, setForcing] = useState(false);
  const [acceptingEula, setAcceptingEula] = useState(false);

  const uptime = useUptime(info?.started_at ?? 0, isLive(info));

  useEffect(() => {
    setError(null);
  }, [detailServerId]);

  if (!server) {
    return (
      <EmptyState
        icon={<ServerIcon className="size-6" />}
        title="No server open"
        description="Pick a server from the list."
      />
    );
  }

  const live = isLive(info);
  const run = async (action: Promise<unknown>) => {
    setBusy(true);
    setError(null);
    try {
      await action;
    } catch (cause) {
      setError(String(cause));
    } finally {
      setBusy(false);
    }
  };

  const acceptEula = async () => {
    setAcceptingEula(true);
    try {
      await api.acceptServerEula(server.id);
      await useStore.getState().refreshServers();
    } catch (cause) {
      setError(String(cause));
    } finally {
      setAcceptingEula(false);
    }
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex flex-wrap items-center gap-4 border-b border-border-soft px-8 py-5">
        <span className="grid size-11 shrink-0 place-items-center rounded-2xl border border-border-soft bg-surface-3 text-content-muted">
          <ServerIcon className="size-5" />
        </span>

        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <h1 className="truncate font-display text-xl font-semibold tracking-tight text-content">
              {server.name}
            </h1>
            <ServerStatusPill server={server} info={info} />
          </div>
          <div className="mt-1 flex flex-wrap items-center gap-x-2 gap-y-1 text-[11px] text-content-muted">
            <span>{flavorLabel(server.flavor)}</span>
            {server.flavor_version && (
              <>
                <span className="text-content-faint">·</span>
                <span>build {server.flavor_version}</span>
              </>
            )}
            <span className="text-content-faint">·</span>
            <span>{server.version_id}</span>
            <span className="text-content-faint">·</span>
            <button
              onClick={() => void navigator.clipboard.writeText(serverAddress(server))}
              title="Copy the address"
              className="font-mono text-content-muted underline decoration-dotted underline-offset-2 hover:text-content"
            >
              {serverAddress(server)}
            </button>
            {live && (
              <>
                <span className="text-content-faint">·</span>
                <span>up {uptime}</span>
              </>
            )}
          </div>
        </div>

        {live && <UsageMeter samples={usage ?? EMPTY_USAGE} maxMemoryMb={server.max_memory_mb} />}

        <div className="flex shrink-0 items-center gap-2">
          <button
            onClick={() => openFolder(server.dir)}
            disabled={!server.available}
            title="Open the folder"
            className="grid size-9 place-items-center rounded-lg border border-border bg-surface-2 text-content-muted transition-colors hover:bg-surface-3 hover:text-content disabled:cursor-not-allowed disabled:opacity-50"
          >
            <FolderOpen className="size-4" />
          </button>

          {!server.installed_at ? (
            <button
              onClick={() => void run(installServer(server.id))}
              disabled={busy || !server.available}
              className="inline-flex items-center gap-2 rounded-lg bg-(--accent) px-4 py-2 text-sm font-semibold text-black transition-opacity disabled:cursor-not-allowed disabled:opacity-45"
            >
              {busy ? <Loader2 className="size-4 animate-spin" /> : <Download className="size-4" />}
              Install
            </button>
          ) : live ? (
            <>
              <button
                onClick={() => void run(stopServer(server.id))}
                disabled={busy}
                className="inline-flex items-center gap-2 rounded-lg border border-border bg-surface-2 px-4 py-2 text-sm font-semibold text-content transition-colors hover:bg-surface-3 disabled:cursor-not-allowed disabled:opacity-50"
              >
                <CircleStop className="size-4" />
                {info?.state === "stopping" ? "Stopping" : "Stop"}
              </button>
              <button
                onClick={() => setForcing(true)}
                title="Kill the process without letting it save"
                className="grid size-9 place-items-center rounded-lg border border-danger/40 bg-danger/10 text-danger transition-colors hover:bg-danger/20"
              >
                <Zap className="size-4" />
              </button>
            </>
          ) : (
            <button
              onClick={() => void run(startServer(server.id))}
              disabled={busy || !server.available}
              className="inline-flex items-center gap-2 rounded-lg bg-(--accent) px-4 py-2 text-sm font-semibold text-black transition-opacity disabled:cursor-not-allowed disabled:opacity-45"
            >
              {busy ? <Loader2 className="size-4 animate-spin" /> : <Play className="size-4" />}
              Start
            </button>
          )}
        </div>
      </div>

      {!server.available && (
        <div className="mx-8 mt-4 rounded-xl border border-danger/30 bg-danger/10 px-4 py-2.5 text-sm text-danger">
          Basalt cannot reach {server.dir} right now. Plug the drive back in, then reopen Basalt.
        </div>
      )}

      {server.available && !server.eula_accepted_at && (
        <div className="mx-8 mt-4 flex flex-wrap items-center gap-3 rounded-xl border border-warn/30 bg-warn/10 px-4 py-2.5 text-sm text-warn">
          <span className="min-w-0 flex-1">
            This server has not accepted the Minecraft EULA, so it will not start.
          </span>
          <button
            onClick={() => void acceptEula()}
            disabled={acceptingEula}
            className="shrink-0 rounded-lg border border-warn/40 px-3 py-1.5 text-[11px] font-semibold text-warn transition-colors hover:bg-warn/15"
          >
            Accept the EULA
          </button>
        </div>
      )}

      {error && (
        <div className="mx-8 mt-4 wrap-break-word rounded-xl border border-danger/30 bg-danger/10 px-4 py-2.5 text-sm text-danger">
          {error}
        </div>
      )}

      <ConsolePanel serverId={server.id} live={live} attached={info?.attached ?? false} />

      <ConfirmDialog
        open={forcing}
        tone="warn"
        title="Kill this server?"
        description="The process is stopped immediately, so anything the world has not saved is lost. Use this only when a graceful stop is stuck."
        confirmLabel="Kill it"
        onCancel={() => setForcing(false)}
        onConfirm={async () => {
          await run(forceStopServer(server.id));
          setForcing(false);
        }}
      />
    </div>
  );
}
