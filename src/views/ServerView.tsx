import { useEffect, useState } from "react";
import {
  CircleStop,
  ClipboardCopy,
  Download,
  FolderOpen,
  Loader2,
  Play,
  RotateCw,
  Server as ServerIcon,
  Zap,
} from "lucide-react";

import { ConfirmDialog } from "../components/ConfirmDialog";
import { ContextMenu, useContextMenu } from "../components/ContextMenu";
import { ConsolePanel } from "../components/servers/ConsolePanel";
import { FilesPanel } from "../components/servers/FilesPanel";
import { PropertiesPanel } from "../components/servers/PropertiesPanel";
import { ServerSettingsPanel } from "../components/servers/ServerSettingsPanel";
import { ServerStatusPill } from "../components/servers/ServerStatusPill";
import { ConsoleStats } from "../components/servers/ConsoleStats";
import { StatsSidebar } from "../components/servers/StatsSidebar";
import { UsageMeter } from "../components/servers/UsageMeter";
import { api } from "../lib/api";
import { cn } from "../lib/cn";
import { openFolder } from "../lib/reveal";
import { flavorLabel, isLive, lanAddress, serverAddress } from "../lib/servers";
import { useUptime } from "../lib/useUptime";
import { EmptyState } from "../components/ui";
import { useStore } from "../store";

const EMPTY_USAGE: never[] = [];
const DISK_POLL_MS = 30000;

const TABS = [
  { id: "console", label: "Console" },
  { id: "files", label: "Files" },
  { id: "properties", label: "Properties" },
  { id: "settings", label: "Settings" },
] as const;

type ServerTab = (typeof TABS)[number]["id"];

export function ServerView() {
  const detailServerId = useStore((s) => s.detailServerId);
  const servers = useStore((s) => s.servers);
  const serverRunning = useStore((s) => s.serverRunning);
  const startServer = useStore((s) => s.startServer);
  const stopServer = useStore((s) => s.stopServer);
  const forceStopServer = useStore((s) => s.forceStopServer);
  const restartServer = useStore((s) => s.restartServer);
  const installServer = useStore((s) => s.installServer);
  const layout = useStore((s) => s.settings?.server_console_layout ?? "sidebar");

  const server = servers.find((entry) => entry.id === detailServerId);
  const usage = useStore((s) => (detailServerId ? s.serverUsage[detailServerId] : undefined));
  const info = detailServerId ? serverRunning[detailServerId] : undefined;

  const [tab, setTab] = useState<ServerTab>("console");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [forcing, setForcing] = useState(false);
  const [acceptingEula, setAcceptingEula] = useState(false);
  const [disk, setDisk] = useState<number | null>(null);
  const [lan, setLan] = useState<string | null>(null);
  const { menu, open: openMenu, close: closeMenu } = useContextMenu();

  const uptime = useUptime(info?.started_at ?? 0, isLive(info));

  useEffect(() => {
    let alive = true;
    void api
      .getLanAddress()
      .then((host) => alive && setLan(host))
      .catch(() => alive && setLan(null));
    return () => {
      alive = false;
    };
  }, []);

  useEffect(() => {
    setError(null);
    setDisk(null);
    if (!detailServerId) return;
    let alive = true;
    const measure = () =>
      api
        .getServerDiskUsage(detailServerId)
        .then((bytes) => alive && setDisk(bytes))
        .catch(() => alive && setDisk(null));
    void measure();
    const timer = setInterval(measure, DISK_POLL_MS);
    return () => {
      alive = false;
      clearInterval(timer);
    };
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
      <div className="flex flex-wrap items-center gap-x-3 gap-y-1.5 border-b border-border-soft px-8 py-2.5">
        <h1 className="wrap-break-word font-display text-lg font-semibold tracking-tight text-content">
          {server.name}
        </h1>
        <ServerStatusPill server={server} info={info} />
        <span className="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-0.5 text-[11px] text-content-muted">
          <span>{flavorLabel(server.flavor)}</span>
          {server.flavor_version && (
            <>
              <span className="text-content-faint">·</span>
              <span>build {server.flavor_version}</span>
            </>
          )}
          <span className="text-content-faint">·</span>
          <span>{server.version_id}</span>
          {layout !== "sidebar" && (
            <>
              <span className="text-content-faint">·</span>
              <button
                onClick={(event) =>
                  openMenu(
                    event,
                    [
                      {
                        label: serverAddress(server),
                        icon: ClipboardCopy,
                        onSelect: () =>
                          void navigator.clipboard.writeText(serverAddress(server)),
                      },
                      ...(lanAddress(server, lan)
                        ? [
                            {
                              label: lanAddress(server, lan)!,
                              icon: ClipboardCopy,
                              onSelect: () =>
                                void navigator.clipboard.writeText(lanAddress(server, lan)!),
                            },
                          ]
                        : []),
                    ],
                    "Copy an address",
                    { below: true },
                  )
                }
                title="Show the addresses"
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
            </>
          )}
        </span>

        <div className="ml-auto flex shrink-0 items-center gap-5">
          {layout === "header" && (
            <UsageMeter
              samples={usage ?? EMPTY_USAGE}
              maxMemoryMb={server.max_memory_mb}
              diskBytes={disk}
              live={live}
            />
          )}
          <div className="flex items-center gap-2">
          <button
            onClick={() => openFolder(server.dir)}
            disabled={!server.available}
            title="Open the folder"
            className="grid size-8 place-items-center rounded-lg border border-border bg-surface-2 text-content-muted transition-colors hover:bg-surface-3 hover:text-content disabled:cursor-not-allowed disabled:opacity-50"
          >
            <FolderOpen className="size-3.5" />
          </button>

          {!server.installed_at ? (
            <button
              onClick={() => void run(installServer(server.id))}
              disabled={busy || !server.available}
              className="inline-flex items-center gap-1.5 rounded-lg bg-(--accent) px-3 py-1.5 text-[13px] font-semibold text-black transition-opacity disabled:cursor-not-allowed disabled:opacity-45"
            >
              {busy ? (
                <Loader2 className="size-3.5 animate-spin" />
              ) : (
                <Download className="size-3.5" />
              )}
              Install
            </button>
          ) : live ? (
            <>
              <button
                onClick={() => void run(restartServer(server.id))}
                disabled={busy || info?.state === "stopping"}
                title="Stop it and start it again"
                className="inline-flex items-center gap-1.5 rounded-lg border border-border bg-surface-2 px-3 py-1.5 text-[13px] font-semibold text-content transition-colors hover:bg-surface-3 disabled:cursor-not-allowed disabled:opacity-50"
              >
                <RotateCw className="size-3.5" />
                Restart
              </button>
              <button
                onClick={() => void run(stopServer(server.id))}
                disabled={busy}
                className="inline-flex items-center gap-1.5 rounded-lg border border-border bg-surface-2 px-3 py-1.5 text-[13px] font-semibold text-content transition-colors hover:bg-surface-3 disabled:cursor-not-allowed disabled:opacity-50"
              >
                <CircleStop className="size-3.5" />
                {info?.state === "stopping" ? "Stopping" : "Stop"}
              </button>
              <button
                onClick={() => setForcing(true)}
                title="Kill the process without letting it save"
                className="grid size-8 place-items-center rounded-lg border border-danger/40 bg-danger/10 text-danger transition-colors hover:bg-danger/20"
              >
                <Zap className="size-3.5" />
              </button>
            </>
          ) : (
            <button
              onClick={() => void run(startServer(server.id))}
              disabled={busy || !server.available}
              className="inline-flex items-center gap-1.5 rounded-lg bg-(--accent) px-3 py-1.5 text-[13px] font-semibold text-black transition-opacity disabled:cursor-not-allowed disabled:opacity-45"
            >
              {busy ? <Loader2 className="size-3.5 animate-spin" /> : <Play className="size-3.5" />}
              Start
            </button>
          )}
          </div>
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

      <div className="flex items-center gap-1 border-b border-border-soft px-6 pt-1">
        {TABS.map((entry) => (
          <button
            key={entry.id}
            onClick={() => setTab(entry.id)}
            className={cn(
              "relative rounded-t-lg px-3.5 py-2 text-[13px] font-medium transition-colors",
              tab === entry.id
                ? "text-content"
                : "text-content-faint hover:text-content-muted",
            )}
          >
            {entry.label}
            {tab === entry.id && (
              <span className="absolute inset-x-2 -bottom-px h-0.5 rounded-full bg-(--accent)" />
            )}
          </button>
        ))}
      </div>

      {tab === "console" && (
        <div className="flex min-h-0 flex-1 flex-col">
          <div className="flex min-h-0 flex-1">
            <ConsolePanel serverId={server.id} live={live} attached={info?.attached ?? false} />
            {layout === "sidebar" && (
              <StatsSidebar
                server={server}
                samples={usage ?? EMPTY_USAGE}
                diskBytes={disk}
                live={live}
                uptime={uptime}
                lan={lan}
              />
            )}
          </div>
          {layout === "below" && (
            <ConsoleStats server={server} samples={usage ?? EMPTY_USAGE} live={live} />
          )}
        </div>
      )}
      {tab === "files" && <FilesPanel server={server} />}
      {tab === "properties" && <PropertiesPanel server={server} live={live} />}
      {tab === "settings" && <ServerSettingsPanel server={server} live={live} />}

      <ContextMenu menu={menu} onClose={closeMenu} />

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
