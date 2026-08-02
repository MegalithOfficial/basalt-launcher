import { useEffect, useState } from "react";
import { ArrowRight, Loader2, Minus, Plus, RefreshCw, ShieldCheck } from "lucide-react";

import { api } from "../lib/api";
import { cn } from "../lib/cn";
import type { Changelog, Instance, ModpackUpgradePlan, ProjectVersion } from "../lib/types";
import { Select } from "./Select";
import { Markdown } from "./project/Markdown";
import { Modal, ModalBody, ModalFooter, ModalHeader } from "./Modal";
import { Toggle } from "./ui";

type Row =
  | { kind: "added" | "changed" | "removed"; path: string }
  | { kind: "replaced"; path: string; from: string };

const LOADERS = new Set(["fabric", "forge", "neoforge", "quilt", "modern", "client"]);

function versionish(token: string) {
  return /^v?\d/.test(token) || /^mc\d/.test(token) || /^\d/.test(token);
}

function modKey(path: string) {
  const name = path.split("/").pop() ?? path;
  const stem = name.replace(/\.(jar|zip|litemod)$/i, "").toLowerCase();
  const tokens = stem.split(/[-_+]/).filter(Boolean);
  const head: string[] = [];
  for (const token of tokens) {
    if (versionish(token)) break;
    head.push(token);
  }
  while (head.length > 1 && LOADERS.has(head[head.length - 1])) head.pop();
  return head.join("-");
}

function pair(added: string[], removed: string[]) {
  const byKey = new Map<string, string[]>();
  for (const path of removed) {
    const key = modKey(path);
    if (!key) continue;
    byKey.set(key, [...(byKey.get(key) ?? []), path]);
  }

  const replaced: Array<{ path: string; from: string }> = [];
  const stillAdded: string[] = [];
  const consumed = new Set<string>();

  for (const path of added) {
    const key = modKey(path);
    const matches = key ? byKey.get(key) : undefined;
    const from = matches?.find((candidate) => !consumed.has(candidate));
    if (from) {
      consumed.add(from);
      replaced.push({ path, from });
    } else {
      stillAdded.push(path);
    }
  }

  return {
    replaced,
    added: stillAdded,
    removed: removed.filter((path) => !consumed.has(path)),
  };
}

const ROW_STYLE = {
  added: { icon: Plus, tone: "text-ok" },
  changed: { icon: RefreshCw, tone: "text-(--accent)" },
  replaced: { icon: RefreshCw, tone: "text-(--accent)" },
  removed: { icon: Minus, tone: "text-danger" },
} as const;

function shorten(path: string) {
  const parts = path.split("/");
  return parts.length > 1 ? parts[parts.length - 1] : path;
}

function folder(path: string) {
  const parts = path.split("/");
  return parts.length > 1 ? parts.slice(0, -1).join("/") : "";
}

export function ModpackUpgradeModal({
  instance,
  plan,
  busy,
  onUpgrade,
  onPickVersion,
  replanning,
  onClose,
}: {
  instance: Instance;
  plan: ModpackUpgradePlan | null;
  busy: boolean;
  onUpgrade: (snapshotFirst: boolean) => void;
  onPickVersion: (versionId: string) => void;
  replanning: boolean;
  onClose: () => void;
}) {
  const [snapshotFirst, setSnapshotFirst] = useState(false);
  const [versions, setVersions] = useState<ProjectVersion[]>([]);
  const [changelog, setChangelog] = useState<Changelog | null>(null);
  const [loadingLog, setLoadingLog] = useState(false);

  const target = plan?.update.target_version_id;
  const provider = instance.pack_provider;
  const project = instance.pack_project_id;

  useEffect(() => {
    if (!provider || !project) return;
    let live = true;
    api
      .listProjectVersions(provider, project, "modpacks", "", null)
      .then((list) => live && setVersions(list))
      .catch(() => live && setVersions([]));
    return () => {
      live = false;
    };
  }, [provider, project]);

  useEffect(() => {
    if (!target || !provider || !project) return;
    setChangelog(null);
    setLoadingLog(true);
    let live = true;
    api
      .getVersionChangelog(provider, project, target)
      .then((entry) => live && setChangelog(entry.body.trim() ? entry : null))
      .catch(() => live && setChangelog(null))
      .finally(() => live && setLoadingLog(false));
    return () => {
      live = false;
    };
  }, [target, provider, project]);

  if (!plan?.changes) return null;
  const { update, changes } = plan;

  const paired = pair(changes.added, changes.removed);
  const rows: Row[] = [
    ...paired.replaced.map((entry) => ({ ...entry, kind: "replaced" as const })),
    ...changes.changed.map((path) => ({ path, kind: "changed" as const })),
    ...paired.added.map((path) => ({ path, kind: "added" as const })),
    ...paired.removed.map((path) => ({ path, kind: "removed" as const })),
  ];

  const installed = versions.find((entry) => entry.id === update.current_version_id);
  const currentLabel =
    installed?.version_number ?? installed?.name ?? update.current_version_id;
  const choices = versions
    .filter((entry) => entry.id !== update.current_version_id)
    .map((entry) => entry.version_number || entry.name)
    .filter((label, index, all) => label && all.indexOf(label) === index);

  const updatedCount = paired.replaced.length + changes.changed.length;
  const summary = [
    updatedCount > 0 && `${updatedCount} updated`,
    paired.added.length > 0 && `${paired.added.length} added`,
    paired.removed.length > 0 && `${paired.removed.length} removed`,
  ].filter(Boolean) as string[];

  return (
    <Modal
      open
      onClose={onClose}
      size="wide"
      dismissable={!busy}
      className="h-[min(660px,calc(100vh-48px))]"
      labelledBy="pack-upgrade-title"
    >
      <ModalHeader
        id="pack-upgrade-title"
        title={
          <span className="flex items-center gap-2.5">
            <span className="text-sm text-content-muted">{currentLabel}</span>
            <ArrowRight className="size-4 shrink-0 text-content-faint" />
            <span>{update.version_number}</span>
          </span>
        }
        subtitle={`${update.target_name} · Minecraft ${update.game_version}${update.loader ? ` · ${update.loader}` : ""}`}
        onClose={busy ? undefined : onClose}
      />

      {choices.length > 1 && (
        <div className="flex shrink-0 items-center gap-3 border-b border-border-soft px-5 py-3">
          <span className="shrink-0 text-[11px] font-medium text-content-muted">
            Upgrade to
          </span>
          <div className="w-64">
            <Select
              value={update.version_number}
              options={choices}
              onChange={(label) => {
                const picked = versions.find(
                  (entry) => (entry.version_number || entry.name) === label,
                );
                if (picked && picked.id !== update.target_version_id) onPickVersion(picked.id);
              }}
              compact
            />
          </div>
          {replanning && (
            <span className="inline-flex items-center gap-1.5 text-[11px] text-content-muted">
              <Loader2 className="size-3.5 animate-spin" />
              Reading that version
            </span>
          )}
        </div>
      )}

      <div className="flex shrink-0 items-center gap-3 border-b border-border-soft px-5 py-3 text-xs">
        <span className="font-medium text-content">
          {summary.length > 0 ? summary.join(" · ") : "No file changes"}
        </span>
        {changes.unchanged > 0 && (
          <span className="text-content-faint">{changes.unchanged} untouched</span>
        )}
        {changes.preserved.length > 0 && (
          <span className="ml-auto inline-flex items-center gap-1.5 text-ok">
            <ShieldCheck className="size-3.5" />
            {changes.preserved.length} edited{" "}
            {changes.preserved.length === 1 ? "file stays" : "files stay"} as yours
          </span>
        )}
      </div>

      <ModalBody className="flex flex-col gap-5">
        {(loadingLog || changelog) && (
          <section>
            <h3 className="mb-2 text-[11px] font-semibold uppercase tracking-wider text-content-faint">
              What the author changed
            </h3>
            {loadingLog ? (
              <div className="flex items-center gap-2 text-xs text-content-muted">
                <Loader2 className="size-3.5 animate-spin" />
                Reading the changelog
              </div>
            ) : (
              <div className="max-h-56 overflow-y-auto rounded-xl border border-border-soft bg-surface-2/40 px-4 py-3">
                <Markdown body={changelog!.body} format={changelog!.format} />
              </div>
            )}
          </section>
        )}

        {rows.length > 0 && (
          <section className="flex min-h-0 flex-col">
            <h3 className="mb-2 text-[11px] font-semibold uppercase tracking-wider text-content-faint">
              Files
            </h3>
            <div className="flex flex-col gap-1">
              {rows.map((row) => {
                const style = ROW_STYLE[row.kind];
                const Icon = style.icon;
                const parent = folder(row.path);
                return (
                  <div
                    key={`${row.kind}:${row.path}`}
                    className="flex items-center gap-2.5 rounded-lg px-2 py-1.5 hover:bg-surface-2/60"
                  >
                    <Icon className={cn("size-3.5 shrink-0", style.tone)} />
                    {row.kind === "replaced" ? (
                      <span className="flex min-w-0 flex-1 items-center gap-2 text-xs">
                        <span className="min-w-0 truncate text-content-faint line-through">
                          {shorten(row.from)}
                        </span>
                        <ArrowRight className="size-3 shrink-0 text-content-faint" />
                        <span className="min-w-0 truncate text-content">{shorten(row.path)}</span>
                      </span>
                    ) : (
                      <span className="min-w-0 flex-1 truncate text-xs text-content">
                        {shorten(row.path)}
                      </span>
                    )}
                    {parent && (
                      <span className="shrink-0 font-mono text-[10px] text-content-faint">
                        {parent}
                      </span>
                    )}
                  </div>
                );
              })}
            </div>
          </section>
        )}
      </ModalBody>

      <ModalFooter className="justify-between">
        <div className="flex items-center gap-2.5">
          <Toggle
            checked={snapshotFirst}
            onChange={setSnapshotFirst}
            disabled={busy}
            label="Take a snapshot first"
          />
          <span className="text-[11px] leading-tight text-content-muted">
            <span className="block font-medium text-content">Snapshot first</span>
            {snapshotFirst
              ? "Slower, but you can roll the upgrade back"
              : "Faster, but this cannot be undone"}
          </span>
        </div>
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={onClose}
            disabled={busy}
            className="h-9 rounded-lg px-3.5 text-xs font-medium text-content-muted transition-colors hover:bg-surface-3 hover:text-content disabled:opacity-50"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={() => onUpgrade(snapshotFirst)}
            disabled={busy || replanning}
            className="inline-flex h-9 items-center gap-2 rounded-lg px-4 text-xs font-semibold text-black shadow-md shadow-(color:--accent-glow) transition-all [background:linear-gradient(to_bottom,var(--accent),var(--accent-deep))] hover:[background:linear-gradient(to_bottom,var(--accent-bright),var(--accent))] disabled:cursor-not-allowed disabled:opacity-50"
          >
            {busy && <Loader2 className="size-3.5 animate-spin" />}
            Upgrade
          </button>
        </div>
      </ModalFooter>
    </Modal>
  );
}
