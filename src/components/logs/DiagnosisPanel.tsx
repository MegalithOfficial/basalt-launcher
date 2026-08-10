import { useEffect, useState } from "react";
import { ChevronDown, Download, FolderOpen, MemoryStick, Search, Stethoscope } from "lucide-react";
import { toast } from "sonner";

import { api } from "../../lib/api";
import { cn } from "../../lib/cn";
import { log } from "../../lib/log";
import { openFolder } from "../../lib/reveal";
import type { Diagnosis, DiagnosisFix, Instance } from "../../lib/types";
import { useStore } from "../../store";

function fixLabel(fix: DiagnosisFix) {
  if (fix === "open_mods_folder") return { label: "Open mods folder", icon: FolderOpen };
  if (typeof fix === "object" && "install_java" in fix)
    return { label: `Install Java ${fix.install_java.major}`, icon: Download };
  if (typeof fix === "object" && "find_content" in fix)
    return { label: `Find ${fix.find_content.query}`, icon: Search };
  if (typeof fix === "object" && "raise_memory" in fix)
    return { label: `Raise to ${fix.raise_memory.megabytes} MB`, icon: MemoryStick };
  return null;
}

export function DiagnosisPanel({
  instance,
  logName = null,
  crash = false,
}: {
  instance: Instance;
  logName?: string | null;
  crash?: boolean;
}) {
  const openDiscover = useStore((s) => s.openDiscover);
  const resetDiscoverBrowse = useStore((s) => s.resetDiscoverBrowse);
  const refreshInstances = useStore((s) => s.refreshInstances);

  const [found, setFound] = useState<Diagnosis[]>([]);
  const [open, setOpen] = useState(true);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let live = true;
    api
      .diagnoseInstance(instance.id, logName, crash)
      .then((list) => live && setFound(list))
      .catch(() => live && setFound([]));
    return () => {
      live = false;
    };
  }, [instance.id, logName, crash]);

  const apply = async (fix: DiagnosisFix) => {
    if (fix === "open_mods_folder") {
      openFolder(`${instance.dir}/mods`);
      return;
    }
    if (typeof fix !== "object") return;

    if ("find_content" in fix) {
      resetDiscoverBrowse({ query: fix.find_content.query });
      openDiscover("mods", instance.id);
      return;
    }

    setBusy(true);
    try {
      if ("install_java" in fix) {
        await api.installJavaRuntime(fix.install_java.major, instance.id);
        toast.success(`Java ${fix.install_java.major} is ready`, {
          description: `${instance.name} will use it on the next launch.`,
        });
      } else if ("raise_memory" in fix) {
        await api.updateInstance(
          instance.id,
          instance.name,
          instance.min_memory_mb,
          fix.raise_memory.megabytes,
          instance.java_path,
          instance.loader,
          instance.loader_version,
          instance.version_id,
          instance.jvm_args,
          instance.jvm_args_mode,
          instance.env_vars,
          instance.env_vars_mode,
        );
        toast.success(`${instance.name} can now use ${fix.raise_memory.megabytes} MB`);
      }
      await refreshInstances();
    } catch (cause) {
      log.warn("diagnose", `could not apply the fix: ${String(cause)}`);
      toast.error("That did not work", { description: String(cause) });
    } finally {
      setBusy(false);
    }
  };

  if (found.length === 0) return null;

  return (
    <div className="shrink-0 border-b border-border-soft bg-surface/60">
      <button
        onClick={() => setOpen((value) => !value)}
        className="flex w-full items-center gap-2 px-4 py-2 text-left"
      >
        <Stethoscope className="size-3.5 shrink-0 text-warn" />
        <span className="text-[11px] font-semibold uppercase tracking-wide text-warn">
          {found.length === 1 ? "Likely cause" : `${found.length} likely causes`}
        </span>
        <span className="min-w-0 flex-1 truncate text-[11px] text-content-faint">
          {open ? "" : found.map((entry) => entry.title).join(" · ")}
        </span>
        <ChevronDown
          className={cn(
            "size-3.5 shrink-0 text-content-faint transition-transform",
            open && "rotate-180",
          )}
        />
      </button>

      {open && (
        <div className="flex flex-col gap-2 px-4 pb-3">
          {found.map((entry) => {
            const action = fixLabel(entry.fix);
            return (
              <div
                key={entry.id}
                className="rounded-xl border border-border-soft bg-surface-2/50 px-3.5 py-3"
              >
                <div className="flex items-start gap-3">
                  <div className="min-w-0 flex-1">
                    <p className="text-[13px] font-semibold text-content">{entry.title}</p>
                    <p className="mt-0.5 text-[11px] leading-relaxed text-content-muted">
                      {entry.detail}
                    </p>
                    {entry.subjects.length > 0 && (
                      <div className="mt-2 flex flex-wrap gap-1.5">
                        {entry.subjects.map((subject) => (
                          <span
                            key={subject}
                            className="rounded border border-border bg-surface-3 px-1.5 py-0.5 font-mono text-[10px] text-content-muted"
                          >
                            {subject}
                          </span>
                        ))}
                      </div>
                    )}
                  </div>
                  {action && (
                    <button
                      onClick={() => void apply(entry.fix)}
                      disabled={busy}
                      className="inline-flex shrink-0 items-center gap-1.5 rounded-lg border border-border bg-surface-2 px-2.5 py-1.5 text-[11px] font-medium text-content-muted transition-colors hover:bg-surface-3 hover:text-content disabled:opacity-50"
                    >
                      <action.icon className="size-3.5" />
                      {action.label}
                    </button>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
