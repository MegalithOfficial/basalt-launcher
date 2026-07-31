import { useCallback, useEffect, useState } from "react";
import { motion } from "motion/react";
import {
  Check,
  FileArchive,
  Loader2,
  Package,
  TriangleAlert,
} from "lucide-react";

import { api } from "../lib/api";
import { cn } from "../lib/cn";
import { LOADERS } from "../lib/loader";
import { formatBytes } from "../lib/packs";
import type { Instance, PackPreview } from "../lib/types";
import { useStore } from "../store";
import { Modal, ModalFooter, ModalHeader } from "./Modal";

function baseName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

function Detail({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0">
      <div className="text-[10px] font-semibold uppercase tracking-wide text-content-faint">
        {label}
      </div>
      <div className="truncate text-[13px] text-content">{value}</div>
    </div>
  );
}

export function ImportPackModal({
  path,
  onClose,
  onImported,
}: {
  path: string | null;
  onClose: () => void;
  onImported?: (instance: Instance) => void;
}) {
  const refreshInstances = useStore((s) => s.refreshInstances);
  const openInstance = useStore((s) => s.openInstance);

  const [preview, setPreview] = useState<PackPreview | null>(null);
  const [name, setName] = useState("");
  const [reading, setReading] = useState(false);
  const [importing, setImporting] = useState(false);
  const [imported, setImported] = useState<Instance | null>(null);
  const [error, setError] = useState<string | null>(null);

  const read = useCallback(async (target: string) => {
    setReading(true);
    setError(null);
    setPreview(null);
    setImported(null);
    try {
      const result = await api.inspectPackFile(target);
      setPreview(result);
      setName(result.name);
    } catch (cause) {
      setError(String(cause));
    } finally {
      setReading(false);
    }
  }, []);

  useEffect(() => {
    if (path) void read(path);
  }, [path, read]);

  const submit = async () => {
    if (!path || !preview) return;
    setImporting(true);
    setError(null);
    try {
      const instance = await api.importPackFile(path, name.trim() || null);
      await refreshInstances();
      setImported(instance);
      onImported?.(instance);
    } catch (cause) {
      setError(String(cause));
    } finally {
      setImporting(false);
    }
  };

  const loaderLabel = preview?.loader
    ? (LOADERS.find((entry) => entry.id === preview.loader)?.label ?? preview.loader)
    : "Vanilla";

  return (
    <Modal
      open={!!path}
      onClose={onClose}
      size="wide"
      className="h-[min(560px,calc(100vh-48px))]"
      dismissable={!importing}
      labelledBy="import-pack-title"
    >
      <ModalHeader
        id="import-pack-title"
        title="Import a modpack file"
        subtitle={path ? baseName(path) : undefined}
        icon={
          <div className="grid size-9 place-items-center rounded-xl border border-border-soft bg-surface-2 text-[var(--accent)]">
            <FileArchive className="size-4" />
          </div>
        }
        onClose={importing ? undefined : onClose}
      />

      <div className="flex min-h-0 flex-1 flex-col px-5 py-4">
        {reading && (
          <div className="flex flex-1 flex-col items-center justify-center gap-3 text-center">
            <Loader2 className="size-5 animate-spin text-[var(--accent)]" />
            <div className="text-sm font-medium text-content">Reading the pack</div>
          </div>
        )}

        {!reading && preview && !imported && (
          <div className="flex min-h-0 flex-1 flex-col gap-4">
            <div className="rounded-xl border border-border-soft bg-surface-2/60 p-4">
              <div className="flex items-start gap-3">
                <span className="grid size-11 shrink-0 place-items-center rounded-xl bg-surface-3 text-content-faint">
                  <Package className="size-5" />
                </span>
                <div className="min-w-0 flex-1">
                  <div className="flex min-w-0 items-center gap-2">
                    <span className="truncate font-display text-[15px] font-semibold text-content">
                      {preview.name || "Untitled pack"}
                    </span>
                    <span className="shrink-0 rounded bg-surface-3 px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-wide text-content-faint">
                      {preview.format === "mrpack" ? "Modrinth" : "CurseForge"}
                    </span>
                  </div>
                  <div className="mt-0.5 truncate text-[11px] text-content-faint">
                    {preview.version ? `Version ${preview.version}` : "No version listed"}
                    {preview.author ? ` · by ${preview.author}` : ""}
                  </div>
                </div>
              </div>

              <div className="mt-4 grid grid-cols-4 gap-3">
                <Detail label="Minecraft" value={preview.game_version || "unknown"} />
                <Detail
                  label="Loader"
                  value={
                    preview.loader_version
                      ? `${loaderLabel} ${preview.loader_version}`
                      : loaderLabel
                  }
                />
                <Detail label="Downloads" value={`${preview.declared_files} files`} />
                <Detail
                  label="Bundled"
                  value={
                    preview.override_files > 0
                      ? `${preview.override_files} files · ${formatBytes(preview.override_bytes)}`
                      : "nothing"
                  }
                />
              </div>
            </div>

            <label className="flex flex-col gap-1.5">
              <span className="text-[11px] font-medium text-content-muted">Instance name</span>
              <input
                value={name}
                onChange={(event) => setName(event.target.value)}
                placeholder={preview.name}
                className="w-full rounded-lg border border-border bg-base px-3 py-2.5 text-sm text-content outline-none focus:border-[var(--accent)]"
              />
            </label>

            <div className="min-h-0 flex-1 overflow-y-auto">
              <div className="flex flex-col gap-1.5">
                {preview.warnings.map((warning) => (
                  <div
                    key={warning}
                    className="flex gap-2.5 rounded-xl border border-warn/25 bg-warn/[0.07] px-3.5 py-2.5 text-xs text-warn"
                  >
                    <TriangleAlert className="mt-0.5 size-4 shrink-0" />
                    <span className="break-words">{warning}</span>
                  </div>
                ))}
              </div>
            </div>
          </div>
        )}

        {imported && (
          <div className="flex flex-1 flex-col items-center justify-center gap-5 text-center">
            <motion.span
              initial={{ scale: 0.6, opacity: 0 }}
              animate={{ scale: 1, opacity: 1 }}
              transition={{ type: "spring", stiffness: 220, damping: 16 }}
              className="grid size-16 place-items-center rounded-2xl bg-ok/15 text-ok"
            >
              <Check className="size-8" strokeWidth={2.5} />
            </motion.span>
            <div>
              <div className="font-display text-xl font-semibold text-content">
                {imported.name} was imported
              </div>
              <p className="mt-1 text-xs text-content-muted">
                {imported.version_id}
                {imported.loader ? ` · ${imported.loader}` : ""}
              </p>
              <p className="mt-3 flex items-center justify-center gap-2 text-xs text-content-faint">
                <Loader2 className="size-3.5 animate-spin text-[var(--accent)]" />
                The files are downloading in the background.
              </p>
            </div>
            <button
              onClick={() => {
                openInstance(imported.id);
                onClose();
              }}
              className="rounded-lg border border-border bg-surface-2 px-3 py-2 text-xs font-medium text-content-muted transition-colors hover:bg-surface-3 hover:text-content"
            >
              Open the instance
            </button>
          </div>
        )}

        {error && (
          <div className="mt-4 flex shrink-0 gap-2.5 rounded-xl border border-danger/25 bg-danger/[0.07] px-3.5 py-3 text-xs text-danger">
            <TriangleAlert className="mt-0.5 size-4 shrink-0" />
            <span className="break-words">{error}</span>
          </div>
        )}
      </div>

      <ModalFooter className="justify-between">
        <span className="text-xs text-content-faint">
          {preview && !imported
            ? "Mods download on import, the rest is copied out of the file."
            : ""}
        </span>
        <div className="flex items-center gap-2">
          <button
            onClick={onClose}
            disabled={importing}
            className="rounded-lg px-3 py-2 text-sm font-medium text-content-muted transition-colors hover:text-content disabled:opacity-50"
          >
            {imported ? "Done" : "Cancel"}
          </button>
          {!imported && (
            <button
              onClick={submit}
              disabled={!preview?.importable || importing || reading}
              className={cn(
                "inline-flex items-center gap-1.5 rounded-lg px-4 py-2 text-sm font-semibold text-black shadow-md shadow-[var(--accent-glow)] transition-all",
                "[background:linear-gradient(to_bottom,var(--accent),var(--accent-deep))] hover:[background:linear-gradient(to_bottom,var(--accent-bright),var(--accent))]",
                "disabled:cursor-not-allowed disabled:opacity-45",
              )}
            >
              {importing && <Loader2 className="size-3.5 animate-spin" />}
              Import pack
            </button>
          )}
        </div>
      </ModalFooter>
    </Modal>
  );
}
