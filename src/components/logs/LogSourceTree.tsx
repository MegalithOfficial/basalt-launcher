import { useCallback, useEffect, useState } from "react";
import {
  ChevronRight,
  FileArchive,
  FileText,
  FileWarning,
  FolderOpen,
  ScrollText,
} from "lucide-react";

import { api } from "../../lib/api";
import { cn } from "../../lib/cn";
import { log } from "../../lib/log";
import { openFolder } from "../../lib/reveal";
import type { InstanceLogFile, RunningInfo } from "../../lib/types";
import { useStore } from "../../store";
import { formatBytes } from "../../lib/format";

export type LogSelection =
  | { kind: "launcher" }
  | { kind: "run"; runningId: string }
  | { kind: "file"; instanceId: string; name: string; crash: boolean };


function Section({ label }: { label: string }) {
  return (
    <div className="px-3 pb-1 pt-3 text-[10px] font-semibold uppercase tracking-wider text-content-faint/70">
      {label}
    </div>
  );
}

function Row({
  active,
  depth = 0,
  onClick,
  children,
}: {
  active: boolean;
  depth?: number;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      onClick={onClick}
      style={{ paddingLeft: 12 + depth * 14 }}
      className={cn(
        "flex w-full items-center gap-2 py-1.5 pr-3 text-left text-xs transition-colors",
        active
          ? "bg-[color-mix(in_srgb,var(--accent)_15%,transparent)] text-content"
          : "text-content-muted hover:bg-surface-2/50 hover:text-content",
      )}
    >
      {children}
    </button>
  );
}

export function LogSourceTree({
  selection,
  onSelect,
  runs,
  issues,
  version,
}: {
  selection: LogSelection;
  onSelect: (selection: LogSelection) => void;
  runs: RunningInfo[];
  issues: number;
  version: number;
}) {
  const instances = useStore((s) => s.instances);

  const [files, setFiles] = useState<Record<string, InstanceLogFile[]>>({});
  const [expanded, setExpanded] = useState<string[]>([]);

  const loadFiles = useCallback(async (instanceId: string) => {
    try {
      const listing = await api.listInstanceLogs(instanceId);
      setFiles((current) => ({ ...current, [instanceId]: listing }));
      return listing;
    } catch (e) {
      log.warn("logs", `could not list instance logs: ${String(e)}`);
      setFiles((current) => ({ ...current, [instanceId]: [] }));
      return [];
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    void Promise.all(
      instances.map(async (instance) => {
        const listing = await loadFiles(instance.id);
        return { id: instance.id, newest: listing[0]?.modified_ms ?? 0 };
      }),
    ).then((results) => {
      if (cancelled) return;
      const best = results.filter((r) => r.newest > 0).sort((a, b) => b.newest - a.newest)[0];
      if (best) setExpanded((current) => (current.length ? current : [best.id]));
    });
    return () => {
      cancelled = true;
    };
  }, [instances, loadFiles, version]);

  const toggle = (instanceId: string) => {
    setExpanded((current) =>
      current.includes(instanceId)
        ? current.filter((id) => id !== instanceId)
        : [...current, instanceId],
    );
    void loadFiles(instanceId);
  };

  return (
    <div className="flex w-64 shrink-0 flex-col overflow-y-auto border-r border-border-soft">
      <Section label="Launcher" />
      <Row active={selection.kind === "launcher"} onClick={() => onSelect({ kind: "launcher" })}>
        <ScrollText className="size-3.5 shrink-0 text-content-faint" />
        <span className="min-w-0 flex-1 truncate font-medium">Launcher</span>
        {issues > 0 && (
          <span className="shrink-0 rounded bg-danger/15 px-1.5 text-[10px] font-semibold text-danger">
            {issues}
          </span>
        )}
      </Row>

      {runs.length > 0 && (
        <>
          <Section label="This session" />
          {runs.map((run) => {
            const name =
              instances.find((i) => i.id === run.instance_id)?.name ?? "Instance";
            return (
              <Row
                key={run.running_id}
                active={selection.kind === "run" && selection.runningId === run.running_id}
                onClick={() => onSelect({ kind: "run", runningId: run.running_id })}
              >
                <span
                  className={cn(
                    "size-1.5 shrink-0 rounded-full",
                    run.state === "running"
                      ? "bg-ok"
                      : run.state === "crashed"
                        ? "bg-danger"
                        : "bg-content-faint",
                  )}
                />
                <span className="min-w-0 flex-1 truncate font-medium">{name}</span>
                <span className="shrink-0 text-[10px] uppercase tracking-wide text-content-faint">
                  {run.state}
                </span>
              </Row>
            );
          })}
        </>
      )}

      <Section label="Logs and crash reports" />
      {instances.length === 0 && (
        <div className="px-3 py-2 text-xs text-content-faint">No instances yet.</div>
      )}
      {instances.map((instance) => {
        const open = expanded.includes(instance.id);
        const listing = files[instance.id];
        const crashes = listing?.filter((file) => file.crash).length ?? 0;
        return (
          <div key={instance.id}>
            <button
              onClick={() => toggle(instance.id)}
              className="flex w-full items-center gap-2 py-1.5 pl-3 pr-3 text-left text-xs text-content-muted transition-colors hover:bg-surface-2/50 hover:text-content"
            >
              <ChevronRight
                className={cn(
                  "size-3.5 shrink-0 text-content-faint transition-transform",
                  open && "rotate-90",
                )}
              />
              <span className="min-w-0 flex-1 truncate font-medium">{instance.name}</span>
              {crashes > 0 && (
                <span className="shrink-0 rounded bg-danger/15 px-1.5 text-[10px] font-semibold text-danger">
                  {crashes}
                </span>
              )}
              <span className="shrink-0 text-[10px] text-content-faint">
                {listing ? listing.length : ""}
              </span>
            </button>

            {open && (
              <>
                {listing?.length === 0 && (
                  <div className="py-1.5 pl-9 pr-3 text-xs text-content-faint">
                    Nothing here yet.
                  </div>
                )}
                {listing?.map((file) => (
                  <Row
                    key={`${file.crash ? "crash" : "log"}/${file.name}`}
                    depth={1}
                    active={
                      selection.kind === "file" &&
                      selection.instanceId === instance.id &&
                      selection.name === file.name &&
                      selection.crash === file.crash
                    }
                    onClick={() =>
                      onSelect({
                        kind: "file",
                        instanceId: instance.id,
                        name: file.name,
                        crash: file.crash,
                      })
                    }
                  >
                    {file.crash ? (
                      <FileWarning className="size-3.5 shrink-0 text-danger" />
                    ) : file.compressed ? (
                      <FileArchive className="size-3.5 shrink-0 text-content-faint" />
                    ) : (
                      <FileText className="size-3.5 shrink-0 text-content-faint" />
                    )}
                    <span className="min-w-0 flex-1 truncate font-mono">{file.name}</span>
                    <span className="shrink-0 tabular-nums text-[10px] text-content-faint">
                      {formatBytes(file.size_bytes)}
                    </span>
                  </Row>
                ))}
                <button
                  onClick={() => openFolder(`${instance.dir}/logs`)}
                  className="flex w-full items-center gap-2 py-1.5 pl-9 pr-3 text-left text-xs text-content-faint transition-colors hover:text-content"
                >
                  <FolderOpen className="size-3.5 shrink-0" />
                  Open folder
                </button>
              </>
            )}
          </div>
        );
      })}
    </div>
  );
}
