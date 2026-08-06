import { useCallback, useEffect, useMemo, useState } from "react";
import { open as openFolderDialog } from "@tauri-apps/plugin-dialog";
import { openPath } from "@tauri-apps/plugin-opener";
import { FolderOpen, HardDrive, Loader2, RotateCcw } from "lucide-react";
import { toast } from "sonner";

import { api } from "../../lib/api";
import { cn } from "../../lib/cn";
import { formatMegabytes } from "../../lib/format";
import { log } from "../../lib/log";
import type { DataLocation, DiskInfo, LocationCandidate } from "../../lib/types";
import { ConfirmDialog } from "../ConfirmDialog";

function freeSpace(disk: DiskInfo | null) {
  if (!disk) return "free space unknown";
  return `${formatMegabytes(disk.free_mb)} free of ${formatMegabytes(disk.total_mb)}`;
}

function diskName(disk: DiskInfo | null) {
  if (!disk) return "unknown drive";
  return disk.mount_point === "/" ? "system drive" : disk.mount_point;
}

export function DataLocations({ heading = true }: { heading?: boolean }) {
  const [locations, setLocations] = useState<DataLocation[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [pending, setPending] = useState<{
    location: DataLocation;
    candidate: LocationCandidate;
  } | null>(null);

  const load = useCallback(async () => {
    try {
      setLocations(await api.getDataLocations());
    } catch (cause) {
      log.warn("storage", `could not read the data locations: ${String(cause)}`);
      setError(String(cause));
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const disks = useMemo(() => {
    const seen = new Map<string, DiskInfo>();
    for (const location of locations ?? []) {
      if (location.disk) seen.set(location.disk.mount_point, location.disk);
    }
    return [...seen.values()].sort((a, b) => a.mount_point.localeCompare(b.mount_point));
  }, [locations]);

  const pick = async (location: DataLocation) => {
    const chosen = await openFolderDialog({ directory: true, title: `Where should ${location.label.toLowerCase()} live?` });
    if (typeof chosen !== "string") return;
    setBusy(location.slot);
    try {
      const candidate = await api.inspectDataLocation(location.slot, chosen);
      if (!candidate.usable) {
        toast.error("Basalt cannot use that folder", { description: candidate.problem ?? undefined });
        return;
      }
      setPending({ location, candidate });
    } catch (cause) {
      toast.error("Could not check that folder", { description: String(cause) });
    } finally {
      setBusy(null);
    }
  };

  const apply = async (location: DataLocation, path: string | null, moveExisting: boolean) => {
    setBusy(location.slot);
    try {
      await api.setDataLocation(location.slot, path, moveExisting);
      setPending(null);
      toast.success(`${location.label} moved`, { description: path ?? location.default_path });
      await load();
    } catch (cause) {
      toast.error(`${location.label} stayed where it was`, { description: String(cause) });
    } finally {
      setBusy(null);
    }
  };

  if (error) return <p className="py-4 text-sm text-danger">{error}</p>;
  if (!locations) {
    return (
      <div className="flex items-center gap-2 py-4 text-sm text-content-faint">
        <Loader2 className="size-3.5 animate-spin" />
        Reading folders
      </div>
    );
  }

  return (
    <div>
      {heading && (
        <div className="flex items-baseline gap-3">
          <h3 className="font-display text-base font-semibold text-content">
            Where the data lives
          </h3>
          {disks.length > 1 && (
            <span className="text-xs text-content-faint">
              spread across {disks.length} drives
            </span>
          )}
        </div>
      )}

      <div className="mt-3">
        {locations.map((location) => (
          <div
            key={location.slot}
            className="group/row flex flex-wrap items-center gap-x-4 gap-y-1 border-b border-border-soft/60 py-2.5 last:border-b-0"
          >
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-2">
                <span className="text-[13px] font-medium text-content">{location.label}</span>
                {location.custom && (
                  <span className="rounded bg-surface-3 px-1.5 py-0.5 text-[10px] font-medium tracking-wide text-content-muted uppercase">
                    moved
                  </span>
                )}
                {!location.exists && (
                  <span className="text-[11px] text-content-faint">not created yet</span>
                )}
              </div>
              <div className="mt-0.5 font-mono text-[11px] break-all text-content-faint">
                {location.path}
              </div>
            </div>

            <div className="flex shrink-0 items-center gap-1.5 text-[11px] text-content-muted">
              <HardDrive className="size-3.5 text-content-faint" />
              <span>{diskName(location.disk)}</span>
              <span className="text-content-faint">·</span>
              <span className="tabular-nums">{freeSpace(location.disk)}</span>
            </div>

            <div className="flex shrink-0 items-center gap-1">
              <button
                onClick={() => void openPath(location.path)}
                title={`Open ${location.label}`}
                aria-label={`Open ${location.label}`}
                disabled={!location.exists}
                className="grid size-7 place-items-center rounded-md text-content-faint opacity-0 transition-colors hover:bg-surface-3 hover:text-content focus-visible:opacity-100 disabled:opacity-0 group-hover/row:opacity-100"
              >
                <FolderOpen className="size-3.5" />
              </button>
              {location.custom && (
                <button
                  onClick={() => void apply(location, null, true)}
                  title="Move back to the default folder"
                  aria-label={`Move ${location.label} back to the default folder`}
                  disabled={busy != null}
                  className="grid size-7 place-items-center rounded-md text-content-faint transition-colors hover:bg-surface-3 hover:text-content disabled:opacity-40"
                >
                  <RotateCcw className="size-3.5" />
                </button>
              )}
              <button
                onClick={() => void pick(location)}
                disabled={busy != null}
                className={cn(
                  "rounded-lg border border-border bg-surface-2 px-3 py-1.5 text-xs font-medium text-content-muted transition-colors hover:bg-surface-3 hover:text-content disabled:opacity-50",
                )}
              >
                {busy === location.slot ? (
                  <Loader2 className="size-3.5 animate-spin" />
                ) : (
                  "Move"
                )}
              </button>
            </div>
          </div>
        ))}
      </div>

      <ConfirmDialog
        open={pending != null}
        tone="warn"
        title={pending ? `Move ${pending.location.label.toLowerCase()}?` : ""}
        confirmLabel="Move"
        description={
          pending && (
            <span className="block space-y-2">
              <span className="block font-mono text-[11px] break-all text-content-faint">
                {pending.location.path}
                <br />
                <span className="text-content-muted">to {pending.candidate.path}</span>
              </span>
              <span className="block">
                {pending.location.exists
                  ? "Everything in the old folder moves across. Nothing is left behind."
                  : "Nothing has been written there yet, so this only changes where Basalt will look."}
              </span>
              {pending.candidate.occupied && (
                <span className="block text-warn">
                  That folder already has files in it. Basalt will refuse rather than mix them.
                </span>
              )}
              <span className="block text-content-faint">
                {freeSpace(pending.candidate.disk)} on {diskName(pending.candidate.disk)}
              </span>
            </span>
          )
        }
        onConfirm={() =>
          pending ? apply(pending.location, pending.candidate.path, true) : Promise.resolve()
        }
        onCancel={() => setPending(null)}
      />
    </div>
  );
}
