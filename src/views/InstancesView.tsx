import { useEffect, useState } from "react";
import { Boxes, Download, Loader2, Pencil, Plus, Trash2 } from "lucide-react";

import { Button, EmptyState, PageHeader } from "../components/ui";
import { CreateInstanceModal } from "../components/CreateInstanceModal";
import { EditInstanceModal } from "../components/EditInstanceModal";
import { mediaSrc } from "../lib/media";
import type { Instance } from "../lib/types";
import { useStore } from "../store";

export function InstancesView() {
  const instances = useStore((s) => s.instances);
  const installs = useStore((s) => s.installs);
  const installedIds = useStore((s) => s.installedIds);
  const installInstance = useStore((s) => s.installInstance);
  const deleteInstance = useStore((s) => s.deleteInstance);
  const mediaMap = useStore((s) => s.media);
  const loadMedia = useStore((s) => s.loadMedia);

  const [modalOpen, setModalOpen] = useState(false);
  const [editing, setEditing] = useState<Instance | null>(null);

  useEffect(() => {
    instances.forEach((i) => loadMedia(i.id));
  }, [instances, loadMedia]);

  return (
    <div className="flex flex-1 flex-col">
      <PageHeader
        title="Instances"
        subtitle="Each instance is a version with its own game directory."
        actions={
          <Button onClick={() => setModalOpen(true)}>
            <Plus className="size-4" />
            New instance
          </Button>
        }
      />

      {instances.length === 0 ? (
        <EmptyState
          icon={<Boxes className="size-6" />}
          title="No instances yet"
          description="Create an instance to choose a Minecraft version and start playing."
          action={
            <Button onClick={() => setModalOpen(true)}>
              <Plus className="size-4" />
              Create your first instance
            </Button>
          }
        />
      ) : (
        <div className="flex flex-1 flex-col gap-2 overflow-y-auto p-6">
          {instances.map((it) => {
            const install = installs[it.id];
            const installed = installedIds.includes(it.id);
            const percent =
              install && install.total > 0
                ? Math.round((install.completed / install.total) * 100)
                : 0;
            return (
              <div
                key={it.id}
                className="flex items-center gap-4 rounded-xl border border-border bg-surface-2 px-4 py-3"
              >
                {mediaMap[it.id] ? (
                  <img
                    src={mediaSrc(mediaMap[it.id]!)}
                    className="size-10 shrink-0 rounded-lg object-cover"
                    draggable={false}
                  />
                ) : (
                  <div className="grid size-10 shrink-0 place-items-center rounded-lg bg-surface-3 text-content-muted">
                    <Boxes className="size-5" />
                  </div>
                )}
                <div className="min-w-0 flex-1">
                  <div className="truncate font-display font-semibold text-content">{it.name}</div>
                  <div className="truncate text-xs text-content-muted">
                    {it.version_id}
                    {installed && !install && " · installed"}
                  </div>
                </div>

                {install ? (
                  <div className="flex w-40 items-center gap-2">
                    <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-surface-3">
                      <div
                        className="h-full rounded-full bg-lava transition-all"
                        style={{ width: `${percent}%` }}
                      />
                    </div>
                    <Loader2 className="size-4 animate-spin text-content-muted" />
                  </div>
                ) : (
                  <button
                    onClick={() => installInstance(it.id)}
                    className="inline-flex items-center gap-1.5 rounded-lg border border-border bg-surface-3 px-3 py-1.5 text-xs font-medium text-content hover:bg-border"
                  >
                    <Download className="size-3.5" />
                    {installed ? "Reinstall" : "Install"}
                  </button>
                )}

                <button
                  onClick={() => setEditing(it)}
                  title="Edit instance"
                  className="grid size-8 place-items-center rounded-lg text-content-faint transition-colors hover:bg-surface-3 hover:text-content"
                >
                  <Pencil className="size-4" />
                </button>
                <button
                  onClick={() => deleteInstance(it.id)}
                  className="grid size-8 place-items-center rounded-lg text-content-faint transition-colors hover:bg-danger/15 hover:text-danger"
                >
                  <Trash2 className="size-4" />
                </button>
              </div>
            );
          })}
        </div>
      )}

      <CreateInstanceModal open={modalOpen} onClose={() => setModalOpen(false)} onCreated={() => {}} />
      <EditInstanceModal instance={editing} onClose={() => setEditing(null)} />
    </div>
  );
}
