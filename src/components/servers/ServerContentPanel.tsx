import { useCallback, useEffect, useMemo, useState } from "react";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import {
  ChevronDown,
  Compass,
  FileUp,
  FolderOpen,
  Loader2,
  Package,
  Plus,
  Power,
  RefreshCw,
  Search,
  SearchX,
  Trash2,
} from "lucide-react";

import { api } from "../../lib/api";
import { cn } from "../../lib/cn";
import { openFolder } from "../../lib/reveal";
import type { ContentItem, RemovalPlan, SearchProvider, Server } from "../../lib/types";
import { ConfirmDialog } from "../ConfirmDialog";
import { ContentItemCard } from "../content/ContentItemCard";
import { ContextMenu, useContextMenu, type MenuItem } from "../ContextMenu";
import { useContentInstaller } from "../CurseForgeDownloadModal";
import { useStore } from "../../store";

interface Removal {
  item: ContentItem;
  plan: RemovalPlan;
  orphans: string[];
}

export function ServerContentPanel({
  server,
  label,
  live,
}: {
  server: Server;
  label: string;
  live: boolean;
}) {
  const openServerDiscover = useStore((s) => s.openServerDiscover);
  const openProject = useStore((s) => s.openProject);
  const contentInstaller = useContentInstaller();
  const { menu, open: openMenu, close: closeMenu } = useContextMenu();

  const [items, setItems] = useState<ContentItem[]>([]);
  const [filter, setFilter] = useState("");
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);
  const [checking, setChecking] = useState(false);
  const [removal, setRemoval] = useState<Removal | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(
    async (reconcile = false) => {
      setLoading(true);
      try {
        setItems(await api.listServerContent(server.id, reconcile));
        setError(null);
      } catch (cause) {
        setError(String(cause));
      } finally {
        setLoading(false);
      }
    },
    [server.id],
  );

  useEffect(() => {
    void load(true);
  }, [load]);

  const shown = useMemo(() => {
    const needle = filter.trim().toLowerCase();
    if (!needle) return items;
    return items.filter(
      (item) =>
        item.file_name.toLowerCase().includes(needle) ||
        (item.source?.title ?? "").toLowerCase().includes(needle),
    );
  }, [items, filter]);

  const run = async (file: string, action: () => Promise<unknown>) => {
    setBusy(file);
    setError(null);
    try {
      await action();
      await load();
    } catch (cause) {
      setError(String(cause));
    } finally {
      setBusy(null);
    }
  };

  const checkUpdates = async () => {
    setChecking(true);
    setError(null);
    try {
      await api.checkServerContentUpdates(server.id, true);
      await load();
    } catch (cause) {
      setError(String(cause));
    } finally {
      setChecking(false);
    }
  };

  const update = async (item: ContentItem) => {
    const source = item.source;
    if (!source?.project_id || !source.provider) return;
    await run(item.file_name, () =>
      contentInstaller.installContent({
        provider: source.provider as SearchProvider,
        projectId: source.project_id!,
        instanceId: null,
        serverId: server.id,
        kind: "mods",
        gameVersion: server.version_id,
        loader: server.flavor,
        versionId: item.update?.latest_version_id ?? null,
        title: source.title ?? item.file_name,
        iconUrl: source.icon_url,
      }),
    );
  };

  const addFiles = async () => {
    const picked = await openFileDialog({
      multiple: true,
      filters: [{ name: "Jar files", extensions: ["jar"] }],
    });
    const sources = Array.isArray(picked) ? picked : picked ? [picked] : [];
    if (sources.length === 0) return;
    await run("", () => api.addServerContent(server.id, sources));
  };

  const remove = async (item: ContentItem, alsoRemove: string[]) => {
    await run(item.file_name, async () => {
      await api.deleteServerContent(server.id, item.file_name);
      for (const file of alsoRemove) await api.deleteServerContent(server.id, file);
    });
  };

  const askRemove = async (item: ContentItem) => {
    const plan = await api
      .planServerContentRemoval(server.id, item.file_name)
      .catch(() => ({ dependents: [], from_pack: false, orphans: [] }) as RemovalPlan);
    if (plan.dependents.length === 0 && plan.orphans.length === 0) {
      await remove(item, []);
      return;
    }
    setRemoval({ item, plan, orphans: plan.orphans.map((orphan) => orphan.file_name) });
  };

  const addContent = (event: React.MouseEvent) => {
    openMenu(
      event,
      [
        {
          label: `Browse ${label.toLowerCase()}`,
          icon: Compass,
          onSelect: () => openServerDiscover(server.id),
        },
        {
          label: "Add files from disk",
          icon: FileUp,
          onSelect: () => void addFiles(),
        },
      ],
      undefined,
      { below: true },
    );
  };

  const rowMenu = (item: ContentItem): MenuItem[] => {
    const source = item.source;
    const entries: MenuItem[] = [];
    if (source?.provider && source.project_id) {
      entries.push({
        label: "Open project page",
        icon: Compass,
        onSelect: () =>
          openProject(
            source.provider as SearchProvider,
            source.project_id!,
            "mods",
            source.title ?? undefined,
          ),
      });
    }
    entries.push({
      label: item.enabled ? "Disable" : "Enable",
      icon: Power,
      onSelect: () =>
        void run(item.file_name, () => api.toggleServerContent(server.id, item.file_name)),
    });
    entries.push({
      label: "Show in folder",
      icon: FolderOpen,
      onSelect: () => void openFolder(`${server.dir}/${label.toLowerCase()}`),
    });
    entries.push({
      label: "Delete",
      icon: Trash2,
      danger: true,
      onSelect: () => void askRemove(item),
    });
    return entries;
  };

  const outdated = items.filter((item) => item.update).length;
  const disabledCount = items.filter((item) => !item.enabled).length;

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex flex-wrap items-center gap-3 px-6 pb-1 pt-4">
        {items.length > 0 && (
          <div className="relative w-full max-w-xs">
            <Search className="absolute left-3 top-1/2 size-3.5 -translate-y-1/2 text-content-faint" />
            <input
              value={filter}
              onChange={(event) => setFilter(event.target.value)}
              placeholder={`Filter ${label.toLowerCase()}`}
              className="w-full rounded-lg border border-border bg-void py-2 pl-9 pr-3 text-sm text-content outline-none transition-colors focus:border-(--accent)"
            />
          </div>
        )}

        <span className="font-pixel text-[10px] uppercase tracking-[0.28em] text-content-faint">
          {items.length} {label.toLowerCase()}
          {disabledCount > 0 && ` · ${disabledCount} disabled`}
          {outdated > 0 && ` · ${outdated} to update`}
        </span>

        <div className="ml-auto flex items-center gap-2">
          {live && (
            <span className="font-pixel text-[10px] uppercase tracking-[0.22em] text-warn">
              restart to apply
            </span>
          )}
          <button
            onClick={() => void checkUpdates()}
            disabled={checking}
            title="Check for updates"
            aria-label="Check for updates"
            className="grid size-9 place-items-center rounded-lg border border-border bg-surface-2 text-content-faint transition-colors hover:bg-surface-3 hover:text-content disabled:cursor-not-allowed disabled:opacity-40"
          >
            <RefreshCw className={cn("size-3.5", checking && "animate-spin")} />
          </button>
          <button
            onClick={addContent}
            className="inline-flex items-center gap-1.5 rounded-lg px-3 py-2 text-xs font-semibold text-black shadow-md shadow-(color:--accent-glow) transition-all [background:linear-gradient(to_bottom,var(--accent),var(--accent-deep))] hover:[background:linear-gradient(to_bottom,var(--accent-bright),var(--accent))]"
          >
            <Plus className="size-3.5" />
            Add content
            <ChevronDown className="size-3.5 opacity-70" />
          </button>
        </div>
      </div>

      {error && (
        <div className="mx-6 mt-3 wrap-break-word rounded-lg border border-danger/30 bg-danger/10 px-3 py-2 text-[11px] text-danger">
          {error}
        </div>
      )}

      <div className="min-h-0 flex-1 overflow-y-auto px-6 py-4">
        {loading ? (
          <div className="flex items-center justify-center gap-2 py-12 text-sm text-content-muted">
            <Loader2 className="size-4 animate-spin" />
            Loading
          </div>
        ) : items.length === 0 ? (
          <div className="flex flex-col items-center gap-3 py-20 text-center">
            <div className="grid size-12 place-items-center rounded-2xl border border-border-soft bg-surface-2 text-content-faint">
              <Package className="size-6" />
            </div>
            <div className="text-sm font-medium text-content-muted">
              No {label.toLowerCase()} yet
            </div>
            <p className="max-w-sm text-xs text-content-faint">
              Browse Modrinth and CurseForge with Add content, or drop in your own files.
            </p>
            <button
              onClick={addContent}
              className="mt-1 inline-flex items-center gap-1.5 rounded-lg px-3 py-2 text-xs font-semibold text-black shadow-md shadow-(color:--accent-glow) transition-all [background:linear-gradient(to_bottom,var(--accent),var(--accent-deep))] hover:[background:linear-gradient(to_bottom,var(--accent-bright),var(--accent))]"
            >
              <Plus className="size-3.5" />
              Add content
            </button>
          </div>
        ) : shown.length === 0 ? (
          <div className="flex items-center gap-3.5 pb-7 pt-9">
            <div className="grid size-11 shrink-0 place-items-center rounded-xl border border-border-soft bg-surface-2 text-content-faint">
              <SearchX className="size-5" />
            </div>
            <div className="min-w-0 flex-1">
              <div className="truncate text-sm font-medium text-content">
                Nothing installed matches “{filter}”
              </div>
              <div className="mt-0.5 text-xs text-content-faint">
                Searched {items.length} installed {items.length === 1 ? "file" : "files"}.
              </div>
            </div>
            <button
              onClick={() => setFilter("")}
              className="shrink-0 rounded-lg border border-border bg-surface-2 px-3 py-1.5 text-xs font-medium text-content-muted transition-colors hover:bg-surface-3 hover:text-content"
            >
              Clear search
            </button>
          </div>
        ) : (
          <div className="flex flex-col gap-1.5">
            {shown.map((item) => (
              <ContentItemCard
                key={item.file_name}
                item={item}
                busy={busy === item.file_name}
                onOpenProject={(provider, projectId, title) =>
                  openProject(provider, projectId, "mods", title)
                }
                onUpdate={() => void update(item)}
                onToggle={() =>
                  void run(item.file_name, () =>
                    api.toggleServerContent(server.id, item.file_name),
                  )
                }
                onRemove={() => void askRemove(item)}
                onContextMenu={(event) => openMenu(event, rowMenu(item))}
              />
            ))}
          </div>
        )}
      </div>

      <ConfirmDialog
        open={!!removal}
        nested
        tone={removal && removal.plan.dependents.length > 0 ? "danger" : "warn"}
        title={removal ? `Remove ${removal.item.source?.title ?? removal.item.file_name}?` : ""}
        description={
          removal ? (
            removal.plan.dependents.length > 0 ? (
              <>
                <span className="font-medium text-danger">
                  {removal.plan.dependents.join(", ")}
                </span>{" "}
                {removal.plan.dependents.length === 1 ? "requires" : "require"} this file.
                Removing it will likely break the server.
              </>
            ) : (
              "This file brought others in with it."
            )
          ) : null
        }
        cancelLabel="Keep it"
        confirmLabel={
          removal && removal.orphans.length > 0
            ? `Remove ${removal.orphans.length + 1} files`
            : removal && removal.plan.dependents.length > 0
              ? "Remove anyway"
              : "Remove"
        }
        onCancel={() => setRemoval(null)}
        onConfirm={async () => {
          const request = removal;
          setRemoval(null);
          if (request) await remove(request.item, request.orphans);
        }}
      >
        {removal && removal.plan.orphans.length > 0 && (
          <>
            <div className="text-xs font-medium text-content">
              {removal.plan.orphans.length === 1
                ? "It installed one dependency that nothing else needs"
                : `It installed ${removal.plan.orphans.length} dependencies that nothing else needs`}
            </div>
            <div className="mt-2.5 flex flex-col gap-1">
              {removal.plan.orphans.map((orphan) => {
                const checked = removal.orphans.includes(orphan.file_name);
                return (
                  <button
                    key={orphan.file_name}
                    onClick={() =>
                      setRemoval({
                        ...removal,
                        orphans: checked
                          ? removal.orphans.filter((file) => file !== orphan.file_name)
                          : [...removal.orphans, orphan.file_name],
                      })
                    }
                    className="flex items-center gap-2.5 rounded-lg px-1.5 py-1.5 text-left transition-colors hover:bg-surface-2"
                  >
                    <span
                      className={cn(
                        "grid size-4 shrink-0 place-items-center rounded border",
                        checked ? "border-transparent bg-(--accent)" : "border-border",
                      )}
                    >
                      {checked && <span className="size-1.5 rounded-sm bg-black" />}
                    </span>
                    <span className="truncate text-xs text-content-muted">
                      {orphan.title ?? orphan.file_name}
                    </span>
                  </button>
                );
              })}
            </div>
          </>
        )}
      </ConfirmDialog>

      <ContextMenu menu={menu} onClose={closeMenu} />
    </div>
  );
}
