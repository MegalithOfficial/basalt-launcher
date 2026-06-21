import { Boxes, Plus } from "lucide-react";

import { Button, EmptyState, PageHeader } from "../components/ui";
import { useStore } from "../store";

export function InstancesView() {
  const instances = useStore((s) => s.instances);

  return (
    <div className="flex flex-1 flex-col">
      <PageHeader
        title="Instances"
        subtitle="Each instance is a version with its own game directory."
        actions={
          <Button>
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
            <Button>
              <Plus className="size-4" />
              Create your first instance
            </Button>
          }
        />
      ) : (
        <div className="grid flex-1 grid-cols-[repeat(auto-fill,minmax(220px,1fr))] content-start gap-4 overflow-y-auto p-8">
          {instances.map((it) => (
            <div
              key={it.id}
              className="rounded-xl border border-border bg-surface-2 p-4 transition-colors hover:border-border"
            >
              <div className="font-display font-semibold text-content">{it.name}</div>
              <div className="mt-1 text-xs text-content-muted">{it.version_id}</div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
