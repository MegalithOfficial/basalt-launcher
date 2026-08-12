import { useEffect, useState } from "react";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import {
  ChevronRight,
  EllipsisVertical,
  File as FileIcon,
  FileArchive,
  FileCode,
  FileImage,
  FolderOpen,
  FilePlus,
  FolderPlus,
  Folder as FolderIcon,
  Box,
  Database,
  Loader2,
  Package,
  Pencil,
  RefreshCw,
  Trash2,
  Upload,
} from "lucide-react";

import { api } from "../../lib/api";
import { cn } from "../../lib/cn";
import { formatBytes } from "../../lib/format";
import { openFolder } from "../../lib/reveal";
import { formatDateTime } from "../../lib/time";
import type { FileKind, Server, ServerEntry, ServerText } from "../../lib/types";
import { ConfirmDialog } from "../ConfirmDialog";
import { ContextMenu, useContextMenu, type MenuItem } from "../ContextMenu";
import { ConfigEditor } from "./ConfigEditor";

const ICONS: Record<FileKind, typeof FileIcon> = {
  properties: FileCode,
  json: FileCode,
  yaml: FileCode,
  toml: FileCode,
  text: FileIcon,
  jar: Package,
  archive: FileArchive,
  image: FileImage,
  schematic: Box,
  nbt: Database,
};

export function FilesPanel({ server }: { server: Server }) {
  const [path, setPath] = useState("");
  const [entries, setEntries] = useState<ServerEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [file, setFile] = useState<ServerText | null>(null);
  const [renaming, setRenaming] = useState<ServerEntry | null>(null);
  const [renameTo, setRenameTo] = useState("");
  const [removing, setRemoving] = useState<ServerEntry | null>(null);
  const [creating, setCreating] = useState<"folder" | "file" | null>(null);
  const [newName, setNewName] = useState("");
  const [picked, setPicked] = useState<string[]>([]);
  const [bulk, setBulk] = useState(false);
  const { menu, open: openMenu, close: closeMenu } = useContextMenu();

  const load = async (next = path) => {
    setLoading(true);
    setError(null);
    try {
      setEntries(await api.listServerFiles(server.id, next));
      setPath(next);
      setPicked([]);
    } catch (cause) {
      setError(String(cause));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void load("");
  }, [server.id]);

  const openEntry = async (entry: ServerEntry) => {
    if (entry.directory) {
      void load(entry.path);
      return;
    }
    setError(null);
    try {
      setFile(await api.readServerFile(server.id, entry.path));
    } catch (cause) {
      setError(String(cause));
    }
  };

  const reloadFile = async () => {
    if (!file) return;
    setFile(await api.readServerFile(server.id, file.path));
  };

  const upload = async () => {
    const picked = await openFileDialog({ multiple: true });
    const sources = Array.isArray(picked) ? picked : picked ? [picked] : [];
    if (sources.length === 0) return;
    setError(null);
    try {
      await api.uploadServerFiles(server.id, path, sources);
      await load();
    } catch (cause) {
      setError(String(cause));
    }
  };

  const entryMenu = (entry: ServerEntry): MenuItem[] => [
    {
      label: "Rename",
      icon: Pencil,
      onSelect: () => {
        setRenaming(entry);
        setRenameTo(entry.name);
      },
    },
    {
      label: "Delete",
      icon: Trash2,
      danger: true,
      onSelect: () => setRemoving(entry),
    },
  ];

  if (file) {
    return (
      <ConfigEditor
        serverId={server.id}
        file={file}
        onClose={() => setFile(null)}
        onSaved={() => void load()}
        onReload={reloadFile}
      />
    );
  }

  const crumbs = path.split("/").filter(Boolean);

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex flex-wrap items-center gap-2 border-b border-border-soft px-8 py-2">
        <div className="flex min-w-0 flex-1 items-center gap-1 text-[12px]">
          <button
            onClick={() => void load("")}
            className={cn(
              "rounded-md px-1.5 py-1 transition-colors hover:bg-surface-3",
              crumbs.length === 0 ? "text-content" : "text-content-muted",
            )}
          >
            {server.name}
          </button>
          {crumbs.map((crumb, index) => (
            <span key={index} className="flex items-center gap-1">
              <ChevronRight className="size-3 text-content-faint" />
              <button
                onClick={() => void load(crumbs.slice(0, index + 1).join("/"))}
                className={cn(
                  "rounded-md px-1.5 py-1 transition-colors hover:bg-surface-3",
                  index === crumbs.length - 1 ? "text-content" : "text-content-muted",
                )}
              >
                {crumb}
              </button>
            </span>
          ))}
        </div>

        {picked.length > 0 && (
          <button
            onClick={() => setBulk(true)}
            className="inline-flex shrink-0 items-center gap-1.5 rounded-lg border border-danger/40 bg-danger/10 px-2.5 py-1.5 text-[11px] font-medium text-danger transition-colors hover:bg-danger/20"
          >
            <Trash2 className="size-3.5" />
            Delete {picked.length}
          </button>
        )}
        <button
          onClick={() => setCreating("file")}
          className="inline-flex shrink-0 items-center gap-1.5 rounded-lg border border-border bg-surface-2 px-2.5 py-1.5 text-[11px] font-medium text-content-muted transition-colors hover:bg-surface-3 hover:text-content"
        >
          <FilePlus className="size-3.5" />
          New file
        </button>
        <button
          onClick={() => setCreating("folder")}
          className="inline-flex shrink-0 items-center gap-1.5 rounded-lg border border-border bg-surface-2 px-2.5 py-1.5 text-[11px] font-medium text-content-muted transition-colors hover:bg-surface-3 hover:text-content"
        >
          <FolderPlus className="size-3.5" />
          New folder
        </button>
        <button
          onClick={() => void upload()}
          className="inline-flex shrink-0 items-center gap-1.5 rounded-lg border border-border bg-surface-2 px-2.5 py-1.5 text-[11px] font-medium text-content-muted transition-colors hover:bg-surface-3 hover:text-content"
        >
          <Upload className="size-3.5" />
          Upload
        </button>
        <button
          onClick={() => void load()}
          title="Reload this folder"
          className="grid size-8 shrink-0 place-items-center rounded-lg border border-border bg-surface-2 text-content-muted transition-colors hover:bg-surface-3 hover:text-content"
        >
          <RefreshCw className="size-3.5" />
        </button>
        <button
          onClick={() => openFolder(server.dir)}
          title="Open in the file manager"
          className="grid size-8 shrink-0 place-items-center rounded-lg border border-border bg-surface-2 text-content-muted transition-colors hover:bg-surface-3 hover:text-content"
        >
          <FolderOpen className="size-3.5" />
        </button>
      </div>

      {error && (
        <div className="wrap-break-word border-b border-danger/30 bg-danger/10 px-8 py-2 text-[11px] text-danger">
          {error}
        </div>
      )}

      {loading ? (
        <div className="grid flex-1 place-items-center text-content-faint">
          <Loader2 className="size-5 animate-spin" />
        </div>
      ) : entries.length === 0 ? (
        <div className="grid flex-1 place-items-center text-sm text-content-faint">
          This folder is empty.
        </div>
      ) : (
        <div className="min-h-0 flex-1 overflow-y-auto">
          {entries.map((entry) => {
            const Icon = entry.directory ? FolderIcon : ICONS[entry.kind];
            return (
              <div
                key={entry.path}
                onClick={() => void openEntry(entry)}
                onContextMenu={(event) => openMenu(event, entryMenu(entry), entry.name)}
                className="group/row flex w-full cursor-pointer items-center gap-3 border-b border-border-soft/60 px-8 py-2 text-left transition-colors hover:bg-surface-2"
              >
                <input
                  type="checkbox"
                  checked={picked.includes(entry.path)}
                  onClick={(event) => event.stopPropagation()}
                  onChange={(event) =>
                    setPicked((current) =>
                      event.target.checked
                        ? [...current, entry.path]
                        : current.filter((value) => value !== entry.path),
                    )
                  }
                  className="size-3.5 shrink-0 accent-(--accent)"
                />
                <Icon
                  className={cn(
                    "size-4 shrink-0",
                    entry.directory ? "text-(--accent)" : "text-content-faint",
                  )}
                />
                <span className="min-w-0 flex-1 wrap-break-word text-[13px] text-content">
                  {entry.name}
                </span>
                <span className="w-24 shrink-0 text-right font-mono text-[11px] text-content-faint">
                  {entry.directory ? "" : formatBytes(entry.size_bytes)}
                </span>
                <span className="w-40 shrink-0 text-right text-[11px] text-content-faint">
                  {entry.modified_ms > 0 ? formatDateTime(entry.modified_ms) : ""}
                </span>
                <button
                  onClick={(event) => {
                    event.stopPropagation();
                    openMenu(event, entryMenu(entry), entry.name, { fromElement: true });
                  }}
                  aria-label={`Actions for ${entry.name}`}
                  className="grid size-6 shrink-0 place-items-center rounded-md text-content-faint opacity-0 transition-colors hover:bg-surface-3 hover:text-content focus-visible:opacity-100 group-hover/row:opacity-100"
                >
                  <EllipsisVertical className="size-3.5" />
                </button>
              </div>
            );
          })}
        </div>
      )}

      <ConfirmDialog
        open={creating !== null}
        tone="warn"
        title={creating === "file" ? "New file" : "New folder"}
        confirmLabel="Create"
        onCancel={() => {
          setCreating(null);
          setNewName("");
        }}
        onConfirm={async () => {
          const name = newName.trim();
          if (!name) return;
          try {
            if (creating === "file") {
              const created = await api.writeServerFile(
                server.id,
                path ? `${path}/${name}` : name,
                "",
              );
              if (created) setError(created.message);
            } else {
              await api.createServerFolder(server.id, path, name);
            }
            await load();
          } catch (cause) {
            setError(String(cause));
          }
          setCreating(null);
          setNewName("");
        }}
      >
        <input
          value={newName}
          onChange={(event) => setNewName(event.target.value)}
          placeholder={creating === "file" ? "config.yml" : "plugins"}
          className="w-full rounded-lg border border-border bg-void px-3 py-2 text-sm text-content outline-none focus:border-(--accent)"
        />
      </ConfirmDialog>

      <ConfirmDialog
        open={bulk}
        title={`Delete ${picked.length} item${picked.length === 1 ? "" : "s"}?`}
        description="Anything selected is deleted, folders with everything inside them. This cannot be undone."
        confirmLabel="Delete"
        onCancel={() => setBulk(false)}
        onConfirm={async () => {
          for (const target of picked) {
            try {
              await api.deleteServerEntry(server.id, target);
            } catch (cause) {
              setError(String(cause));
            }
          }
          await load();
          setBulk(false);
        }}
      />

      <ConfirmDialog
        open={!!renaming}
        tone="warn"
        title={`Rename ${renaming?.name ?? ""}`}
        confirmLabel="Rename"
        onCancel={() => setRenaming(null)}
        onConfirm={async () => {
          if (!renaming) return;
          try {
            await api.renameServerEntry(server.id, renaming.path, renameTo.trim());
            await load();
          } catch (cause) {
            setError(String(cause));
          }
          setRenaming(null);
        }}
      >
        <input
          value={renameTo}
          onChange={(event) => setRenameTo(event.target.value)}
          className="w-full rounded-lg border border-border bg-void px-3 py-2 text-sm text-content outline-none focus:border-(--accent)"
        />
      </ConfirmDialog>

      <ConfirmDialog
        open={!!removing}
        title={`Delete ${removing?.name ?? ""}?`}
        description={
          removing?.directory
            ? "This folder and everything inside it is deleted. This cannot be undone."
            : "This file is deleted. This cannot be undone."
        }
        requireText={removing?.directory ? removing.name : undefined}
        onCancel={() => setRemoving(null)}
        onConfirm={async () => {
          if (!removing) return;
          try {
            await api.deleteServerEntry(server.id, removing.path);
            await load();
          } catch (cause) {
            setError(String(cause));
          }
          setRemoving(null);
        }}
      />

      <ContextMenu menu={menu} onClose={closeMenu} />
    </div>
  );
}
