import { useCallback, useEffect, useState } from "react";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { motion } from "motion/react";
import { Check, FolderOpen, Loader2, Share, TriangleAlert } from "lucide-react";

import { api } from "../lib/api";
import { cn } from "../lib/cn";
import { formatBytes } from "../lib/format";
import { loaderLabel } from "../lib/loader";
import { PACK_FORMATS, pickPackDestination } from "../lib/packs";
import type { ExportCandidate, Instance, PackExport, PackFormat } from "../lib/types";
import { defaultRules, ExportFileTree, type ExportRules } from "./ExportFileTree";
import { Modal, ModalFooter, ModalHeader } from "./Modal";

const inputCls =
  "w-full rounded-lg border border-border bg-void px-3 py-2 text-sm text-content outline-none transition-colors placeholder:text-content-faint focus:border-(--accent)";
const eyebrowCls = "font-pixel text-[9px] tracking-[0.14em] text-content-faint uppercase";

function rulesKey(instanceId: string) {
  return `export-rules:${instanceId}`;
}

function loadRules(instanceId: string): ExportRules | null {
  try {
    const raw = localStorage.getItem(rulesKey(instanceId));
    if (!raw) return null;
    const parsed = JSON.parse(raw) as Partial<ExportRules>;
    const strings = (list: unknown) =>
      Array.isArray(list) ? list.filter((v): v is string => typeof v === "string") : [];
    return { included: strings(parsed.included), excluded: strings(parsed.excluded) };
  } catch {
    return null;
  }
}

function saveRules(instanceId: string, rules: ExportRules) {
  try {
    localStorage.setItem(rulesKey(instanceId), JSON.stringify(rules));
  } catch {
    return;
  }
}


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
  const [name, setName] = useState("");
  const [version, setVersion] = useState("");
  const [description, setDescription] = useState("");
  const [rules, setRules] = useState<ExportRules | null>(null);

  useEffect(() => {
    if (instance) {
      setFormat("mrpack");
      setResult(null);
      setError(null);
      setName(instance.name);
      setVersion("1.0.0");
      setDescription("");
      setRules(loadRules(instance.id));
    }
  }, [instance]);

  const instanceId = instance?.id ?? null;
  const onTreeLoaded = useCallback(
    (root: ExportCandidate[]) => {
      if (!instanceId) return;
      setRules((current) => current ?? defaultRules(root));
    },
    [instanceId],
  );

  const changeRules = (next: ExportRules) => {
    setRules(next);
    if (instance) saveRules(instance.id, next);
  };

  const submit = async () => {
    if (!instance) return;
    setError(null);
    try {
      const suggested = await api.packExportName(name.trim() || instance.name, format);
      const destination = await pickPackDestination(suggested, format);
      if (!destination) return;
      setBusy(true);
      setResult(
        await api.exportInstancePack(instance.id, format, destination, {
          name: name.trim() || null,
          version: version.trim() || null,
          description: description.trim() || null,
          included: rules?.included ?? [],
          excluded: rules?.excluded ?? [],
        }),
      );
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
      size="xl"
      dismissable={!busy}
      labelledBy="export-pack-title"
    >
      <ModalHeader
        id="export-pack-title"
        title="Export as a modpack"
        subtitle={
          instance
            ? [instance.name, instance.version_id, loaderLabel(instance)].filter(Boolean).join(" · ")
            : undefined
        }
        icon={
          <div className="grid size-9 place-items-center rounded-xl border border-border-soft bg-surface-2 text-(--accent)">
            <Share className="size-4" />
          </div>
        }
        onClose={busy ? undefined : onClose}
      />

      <div className="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto px-5 py-5">
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
              <div className="flex items-center gap-2.5">
                <span className={eyebrowCls}>Format</span>
                <span className="h-px flex-1 bg-border-soft" />
              </div>
              <div className="grid grid-cols-2 gap-2.5">
                {PACK_FORMATS.map((entry) => {
                  const on = format === entry.id;
                  return (
                    <button
                      key={entry.id}
                      onClick={() => setFormat(entry.id)}
                      aria-pressed={on}
                      className={cn(
                        "relative flex flex-col items-start gap-1.5 rounded-xl border px-3.5 py-3 text-left transition-colors",
                        on
                          ? "border-(--accent)/45 bg-surface-3"
                          : "border-border-soft bg-surface-2/60 hover:border-border",
                      )}
                    >
                      <span
                        className={cn(
                          "absolute right-3 top-3 size-2 rounded-[2px] transition-colors",
                          on ? "bg-(--accent)" : "bg-border",
                        )}
                      />
                      <span
                        className={cn(
                          "font-pixel text-lg leading-none tracking-wide",
                          on ? "text-content" : "text-content-muted",
                        )}
                      >
                        .{entry.extension}
                      </span>
                      <span className="font-display text-[13px] font-semibold text-content">
                        {entry.label}
                      </span>
                      <span className="text-[11px] leading-snug text-content-faint">{entry.note}</span>
                    </button>
                  );
                })}
              </div>
            </div>

            <div className="grid grid-cols-[1fr_150px] gap-2.5">
              <label className="flex flex-col gap-1.5">
                <span className={eyebrowCls}>Name</span>
                <input
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder={instance?.name}
                  className={inputCls}
                />
              </label>
              <label className="flex flex-col gap-1.5">
                <span className={eyebrowCls}>Version</span>
                <input
                  value={version}
                  onChange={(e) => setVersion(e.target.value)}
                  placeholder="1.0.0"
                  className={cn(inputCls, "font-mono")}
                />
              </label>
              {format === "mrpack" && (
                <label className="col-span-2 flex flex-col gap-1.5">
                  <span className={eyebrowCls}>Description</span>
                  <input
                    value={description}
                    onChange={(e) => setDescription(e.target.value)}
                    placeholder="Optional, shown by launchers"
                    className={inputCls}
                  />
                </label>
              )}
            </div>

            {instance && (
              <ExportFileTree
                instanceId={instance.id}
                format={format}
                rules={rules ?? { included: [], excluded: [] }}
                onRulesChange={changeRules}
                onLoaded={onTreeLoaded}
              />
            )}
            <p className="text-[11px] leading-relaxed text-content-faint">
              <span className="text-content-muted">
                Checked mods {active?.label} knows travel as a link
              </span>
              , everything else checked is packed into the file. Logs, crash reports and
              launcher state never leave.
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
