import { useState } from "react";
import { CircleStop, FolderOpen, Play, Plus, Server as ServerIcon, Trash2 } from "lucide-react";

import { ConfirmDialog } from "../components/ConfirmDialog";
import { ContextMenu, useContextMenu, type MenuItem } from "../components/ContextMenu";
import { CreateServerModal } from "../components/CreateServerModal";
import { ServerStatusPill } from "../components/servers/ServerStatusPill";
import { cn } from "../lib/cn";
import { openFolder } from "../lib/reveal";
import { flavorLabel, isLive, serverAddress } from "../lib/servers";
import { relativeTime } from "../lib/time";
import type { Server } from "../lib/types";
import { Button, EmptyState, PageHeader } from "../components/ui";
import { useStore } from "../store";

export function ServersView() {
  const servers = useStore((s) => s.servers);
  const serverRunning = useStore((s) => s.serverRunning);
  const openServer = useStore((s) => s.openServer);
  const startServer = useStore((s) => s.startServer);
  const stopServer = useStore((s) => s.stopServer);
  const deleteServer = useStore((s) => s.deleteServer);

  const [creating, setCreating] = useState(false);
  const [removing, setRemoving] = useState<Server | null>(null);
  const [error, setError] = useState<string | null>(null);
  const { menu, open: openMenu, close: closeMenu } = useContextMenu();

  const run = async (action: Promise<unknown>) => {
    setError(null);
    try {
      await action;
    } catch (cause) {
      setError(String(cause));
    }
  };

  const serverMenu = (server: Server): MenuItem[] => {
    const info = serverRunning[server.id];
    return [
      isLive(info)
        ? {
            label: "Stop",
            icon: CircleStop,
            onSelect: () => void run(stopServer(server.id)),
          }
        : {
            label: "Start",
            icon: Play,
            onSelect: () => void run(startServer(server.id)),
          },
      {
        label: "Open folder",
        icon: FolderOpen,
        onSelect: () => openFolder(server.dir),
      },
      {
        label: "Remove",
        icon: Trash2,
        danger: true,
        onSelect: () => setRemoving(server),
      },
    ];
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <PageHeader
        title="Servers"
        subtitle={
          servers.length === 0
            ? "Host a Minecraft server from Basalt."
            : `${servers.length} server${servers.length === 1 ? "" : "s"}`
        }
        actions={
          <Button onClick={() => setCreating(true)}>
            <Plus className="size-4" />
            Add server
          </Button>
        }
      />

      {error && (
        <div className="mx-8 mt-4 rounded-xl border border-danger/30 bg-danger/10 px-4 py-2.5 text-sm text-danger">
          {error}
        </div>
      )}

      {servers.length === 0 ? (
        <EmptyState
          icon={<ServerIcon className="size-6" />}
          title="No servers yet"
          description="Create one from a Minecraft version, or import a folder you already run."
          action={
            <Button onClick={() => setCreating(true)}>
              <Plus className="size-4" />
              Add your first server
            </Button>
          }
        />
      ) : (
        <div className="min-h-0 flex-1 overflow-y-auto px-8 py-5">
          <div className="flex flex-col gap-2">
            {servers.map((server) => {
              const info = serverRunning[server.id];
              return (
                <div
                  key={server.id}
                  onClick={() => openServer(server.id)}
                  onContextMenu={(event) => openMenu(event, serverMenu(server), server.name)}
                  className={cn(
                    "group flex cursor-pointer items-center gap-4 rounded-xl border border-border-soft bg-surface-2/40 px-4 py-3 transition-colors hover:border-border hover:bg-surface-2",
                    !server.available && "opacity-70",
                  )}
                >
                  <span className="grid size-10 shrink-0 place-items-center rounded-xl border border-border-soft bg-surface-3 text-content-muted">
                    <ServerIcon className="size-4.5" />
                  </span>

                  <div className="min-w-0 flex-1">
                    <div className="truncate text-sm font-medium text-content">{server.name}</div>
                    <div className="mt-0.5 flex flex-wrap items-center gap-x-2 gap-y-1 text-[11px] text-content-muted">
                      <span>{flavorLabel(server.flavor)}</span>
                      <span className="text-content-faint">·</span>
                      <span>{server.version_id}</span>
                      <span className="text-content-faint">·</span>
                      <span className="font-mono">{serverAddress(server)}</span>
                      {server.last_started_at && (
                        <>
                          <span className="text-content-faint">·</span>
                          <span>last run {relativeTime(server.last_started_at * 1000)}</span>
                        </>
                      )}
                    </div>
                  </div>

                  <ServerStatusPill server={server} info={info} />

                  <button
                    onClick={(event) => {
                      event.stopPropagation();
                      void run(isLive(info) ? stopServer(server.id) : startServer(server.id));
                    }}
                    disabled={!server.available || !server.installed_at}
                    title={server.installed_at ? undefined : "Install this server first"}
                    className="inline-flex shrink-0 items-center gap-1.5 rounded-lg border border-border bg-surface-2 px-2.5 py-1.5 text-[11px] font-medium text-content-muted transition-colors hover:bg-surface-3 hover:text-content disabled:cursor-not-allowed disabled:opacity-50"
                  >
                    {isLive(info) ? (
                      <>
                        <CircleStop className="size-3.5" />
                        Stop
                      </>
                    ) : (
                      <>
                        <Play className="size-3.5" />
                        Start
                      </>
                    )}
                  </button>
                </div>
              );
            })}
          </div>
        </div>
      )}

      <CreateServerModal
        open={creating}
        onClose={() => setCreating(false)}
        onCreated={(id) => openServer(id)}
      />

      <ConfirmDialog
        open={!!removing}
        title={`Remove ${removing?.name ?? "this server"}?`}
        description={
          removing?.managed
            ? "Basalt deletes this server folder, including its worlds. This cannot be undone."
            : "Basalt forgets this server. The folder and everything in it stays where it is."
        }
        confirmLabel="Remove"
        requireText={removing?.managed ? removing.name : undefined}
        onCancel={() => setRemoving(null)}
        onConfirm={async () => {
          if (!removing) return;
          await run(deleteServer(removing.id, removing.managed));
          setRemoving(null);
        }}
      />

      <ContextMenu menu={menu} onClose={closeMenu} />
    </div>
  );
}
