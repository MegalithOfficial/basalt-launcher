import { useEffect, useState } from "react";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { motion } from "motion/react";
import { Check, FolderOpen, Loader2, Share, TriangleAlert } from "lucide-react";

import { api } from "../lib/api";
import { cn } from "../lib/cn";
import { formatBytes } from "../lib/format";
import { PACK_FORMATS, pickPackDestination } from "../lib/packs";
import type { Instance, PackExport, PackFormat } from "../lib/types";
import { Modal, ModalFooter, ModalHeader } from "./Modal";

export function ExportPackModal({
  instance,
  onClose,
}: {
  instance: Instance | null;
  onClose: () => void;
}) {
  const [format, setFormat] = useState<PackFormat>("mrpack");
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<PackExport | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (instance) {
      setFormat("mrpack");
      setResult(null);
      setError(null);
    }
  }, [instance]);

  const submit = async () => {
    if (!instance) return;
    setError(null);
    try {
      const suggested = await api.packExportName(instance.name, format);
      const destination = await pickPackDestination(suggested, format);
      if (!destination) return;
      setBusy(true);
      setResult(await api.exportInstancePack(instance.id, format, destination));
    } catch (cause) {
      setError(String(cause));
    } finally {
      setBusy(false);
    }
  };

  const active = PACK_FORMATS.find((entry) => entry.id === format);

  return (
    <Modal
      open={!!instance}
      onClose={onClose}
      size="lg"
      dismissable={!busy}
      labelledBy="export-pack-title"
    >
      <ModalHeader
        id="export-pack-title"
        title="Export as a modpack"
        subtitle={instance ? instance.name : undefined}
        icon={
          <div className="grid size-9 place-items-center rounded-xl border border-border-soft bg-surface-2 text-(--accent)">
            <Share className="size-4" />
          </div>
        }
        onClose={busy ? undefined : onClose}
      />

      <div className="flex flex-col gap-4 px-5 py-5">
        {result ? (
          <div className="flex flex-col items-center gap-5 py-4 text-center">
            <motion.span
              initial={{ scale: 0.6, opacity: 0 }}
              animate={{ scale: 1, opacity: 1 }}
              transition={{ type: "spring", stiffness: 220, damping: 16 }}
              className="grid size-16 place-items-center rounded-2xl bg-ok/15 text-ok"
            >
              <Check className="size-8" strokeWidth={2.5} />
            </motion.span>
            <div>
              <div className="font-display text-xl font-semibold text-content">Pack written</div>
              <p className="mt-1 text-xs text-content-muted">
                {result.linked} {result.linked === 1 ? "mod listed" : "mods listed"} by link ·{" "}
                {result.bundled} {result.bundled === 1 ? "file" : "files"} bundled ·{" "}
                {formatBytes(result.bytes)}
              </p>
            </div>
            <button
              onClick={() => void revealItemInDir(result.path)}
              className="inline-flex items-center gap-1.5 rounded-lg border border-border bg-surface-2 px-3 py-2 text-xs font-medium text-content-muted transition-colors hover:bg-surface-3 hover:text-content"
            >
              <FolderOpen className="size-3.5" />
              Show the file
            </button>
          </div>
        ) : (
          <>
            <div className="flex flex-col gap-2">
              {PACK_FORMATS.map((entry) => (
                <button
                  key={entry.id}
                  onClick={() => setFormat(entry.id)}
                  className={cn(
                    "flex items-start gap-3 rounded-xl border px-4 py-3 text-left transition-colors",
                    format === entry.id
                      ? "border-(--accent)/50 bg-(--accent)/[0.07]"
                      : "border-border-soft bg-surface-2/60 hover:border-border",
                  )}
                >
                  <span
                    className={cn(
                      "mt-0.5 grid size-4 shrink-0 place-items-center rounded-full border",
                      format === entry.id
                        ? "border-(--accent) bg-(--accent) text-black"
                        : "border-border bg-surface-3",
                    )}
                  >
                    {format === entry.id && <Check className="size-2.5" strokeWidth={4} />}
                  </span>
                  <span className="min-w-0 flex-1">
                    <span className="flex items-center gap-2">
                      <span className="text-sm font-medium text-content">{entry.label}</span>
                      <span className="rounded bg-surface-3 px-1.5 py-0.5 font-mono text-[9px] text-content-faint">
                        .{entry.extension}
                      </span>
                    </span>
                    <span className="mt-0.5 block text-[11px] text-content-faint">
                      {entry.note}
                    </span>
                  </span>
                </button>
              ))}
            </div>

            <p className="text-[11px] text-content-faint">
              Worlds, logs, screenshots and backups stay out of the file. Anything a{" "}
              {active?.label} link cannot describe travels inside it, so configs and local mods
              come along.
            </p>
          </>
        )}

        {error && (
          <div className="flex gap-2.5 rounded-xl border border-danger/25 bg-danger/[0.07] px-3.5 py-3 text-xs text-danger">
            <TriangleAlert className="mt-0.5 size-4 shrink-0" />
            <span className="wrap-break-word">{error}</span>
          </div>
        )}
      </div>

      <ModalFooter>
        <button
          onClick={onClose}
          disabled={busy}
          className="rounded-lg px-3 py-2 text-sm font-medium text-content-muted transition-colors hover:text-content disabled:opacity-50"
        >
          {result ? "Done" : "Cancel"}
        </button>
        {!result && (
          <button
            onClick={submit}
            disabled={busy}
            className="inline-flex items-center gap-1.5 rounded-lg px-4 py-2 text-sm font-semibold text-black shadow-md shadow-(color:--accent-glow) transition-all [background:linear-gradient(to_bottom,var(--accent),var(--accent-deep))] hover:[background:linear-gradient(to_bottom,var(--accent-bright),var(--accent))] disabled:cursor-not-allowed disabled:opacity-45"
          >
            {busy && <Loader2 className="size-3.5 animate-spin" />}
            Choose a location
          </button>
        )}
      </ModalFooter>
    </Modal>
  );
}
