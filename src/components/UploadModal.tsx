import { useCallback, useEffect, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { FileUp, Folder, FolderOpen, Loader2, TriangleAlert, X } from "lucide-react";

import { api } from "../lib/api";
import { cn } from "../lib/cn";
import { log } from "../lib/log";
import { Modal, ModalFooter, ModalHeader } from "./Modal";

function baseName(path: string) {
  const parts = path.split(/[\\/]/);
  return parts[parts.length - 1] || path;
}

function matches(path: string, extensions: string[]) {
  if (extensions.length === 0) return true;
  const lower = path.toLowerCase();
  return extensions.some((extension) => lower.endsWith(`.${extension.toLowerCase()}`));
}

export function UploadModal({
  open,
  onClose,
  title,
  subtitle,
  extensions,
  filterName,
  multiple = false,
  allowDirectories = false,
  directoryLabel = "Select folder",
  confirmLabel = "Add",
  nested = false,
  busy = false,
  error,
  onConfirm,
}: {
  open: boolean;
  onClose: () => void;
  title: string;
  subtitle?: string;
  extensions: string[];
  filterName: string;
  multiple?: boolean;
  allowDirectories?: boolean;
  directoryLabel?: string;
  confirmLabel?: string;
  nested?: boolean;
  busy?: boolean;
  error?: string | null;
  onConfirm: (paths: string[]) => void;
}) {
  const [picked, setPicked] = useState<string[]>([]);
  const [hovering, setHovering] = useState(false);
  const [rejected, setRejected] = useState(0);

  useEffect(() => {
    if (!open) return;
    setPicked([]);
    setHovering(false);
    setRejected(0);
  }, [open]);

  const accept = useCallback(
    async (paths: string[]) => {
      let kept = paths.filter((path) => matches(path, extensions));

      if (allowDirectories && kept.length < paths.length) {
        const rest = paths.filter((path) => !kept.includes(path));
        const folders = await api
          .inspectPaths(rest)
          .then((seen) => seen.filter((entry) => entry.directory).map((entry) => entry.path))
          .catch((cause) => {
            log.warn("upload", `could not read the dropped paths: ${String(cause)}`);
            return [] as string[];
          });
        kept = paths.filter((path) => kept.includes(path) || folders.includes(path));
      }

      setRejected(paths.length - kept.length);
      if (kept.length === 0) return;
      setPicked((current) => {
        if (!multiple) return [kept[kept.length - 1]];
        return [...current, ...kept.filter((path) => !current.includes(path))];
      });
    },
    [extensions, multiple, allowDirectories],
  );

  useEffect(() => {
    if (!open) return;
    let stop: (() => void) | undefined;
    let live = true;

    getCurrentWebview()
      .onDragDropEvent((event) => {
        if (event.payload.type === "drop") {
          setHovering(false);
          void accept(event.payload.paths);
        } else if (event.payload.type === "leave") {
          setHovering(false);
        } else {
          setHovering(true);
        }
      })
      .then((unlisten) => {
        if (live) stop = unlisten;
        else unlisten();
      })
      .catch((cause) => log.warn("upload", `no drag and drop: ${String(cause)}`));

    return () => {
      live = false;
      stop?.();
    };
  }, [open, accept]);

  const browse = async (directory: boolean) => {
    const chosen = await openFileDialog({
      multiple,
      directory,
      title,
      ...(directory ? {} : { filters: [{ name: filterName, extensions }] }),
    });
    if (!chosen) return;
    await accept(Array.isArray(chosen) ? chosen : [chosen]);
  };

  const remove = (path: string) =>
    setPicked((current) => current.filter((value) => value !== path));

  return (
    <Modal
      open={open}
      onClose={onClose}
      size="lg"
      nested={nested}
      dismissable={!busy}
      labelledBy="upload-title"
    >
      <ModalHeader
        id="upload-title"
        title={title}
        subtitle={subtitle}
        icon={
          <div className="grid size-9 place-items-center rounded-xl border border-border-soft bg-surface-2 text-(--accent)">
            <FileUp className="size-4" />
          </div>
        }
        onClose={busy ? undefined : onClose}
      />

      <div className="px-5 py-5">
        <button
          onClick={() => void browse(false)}
          disabled={busy}
          className={cn(
            "flex w-full flex-col items-center justify-center gap-2 rounded-2xl border-2 border-dashed px-6 py-10 transition-colors disabled:opacity-50",
            hovering
              ? "border-(--accent) bg-(--accent)/[0.06]"
              : "border-border bg-surface-2/40 hover:border-border-soft hover:bg-surface-2/70",
          )}
        >
          <FileUp
            className={cn(
              "size-7 transition-colors",
              hovering ? "text-(--accent)" : "text-content-faint",
            )}
          />
          <span className="text-sm font-medium text-content">
            {hovering
              ? "Drop to add"
              : multiple
                ? "Drop files here"
                : "Drop a file here"}
          </span>
          <span className="text-[11px] text-content-faint">
            or click to browse · {extensions.map((value) => `.${value}`).join(" ")}
            {allowDirectories ? " · folders" : ""}
          </span>
        </button>

        {allowDirectories && (
          <button
            onClick={() => void browse(true)}
            disabled={busy}
            className="mt-2.5 inline-flex w-full items-center justify-center gap-1.5 rounded-lg border border-border bg-surface-2 px-3 py-2 text-xs font-medium text-content-muted transition-colors hover:bg-surface-3 hover:text-content disabled:opacity-50"
          >
            <Folder className="size-3.5" />
            {directoryLabel}
          </button>
        )}

        {error && (
          <div className="mt-3 flex gap-2.5 rounded-xl border border-danger/25 bg-danger/[0.07] px-3.5 py-3 text-xs text-danger">
            <TriangleAlert className="mt-0.5 size-4 shrink-0" />
            <span className="break-words">{error}</span>
          </div>
        )}

        {rejected > 0 && (
          <div className="mt-3 flex items-start gap-2 text-[11px] text-warn">
            <TriangleAlert className="mt-0.5 size-3.5 shrink-0" />
            <span>
              {rejected} {rejected === 1 ? "item was" : "items were"} skipped. Only{" "}
              {extensions.map((value) => `.${value}`).join(", ")}
              {allowDirectories ? " files and folders" : " files"} can go here.
            </span>
          </div>
        )}

        {picked.length > 0 && (
          <div className="mt-4">
            <div className="flex items-baseline justify-between">
              <span className="text-[11px] font-semibold uppercase tracking-wider text-content-faint">
                {multiple ? `${picked.length} selected` : "Selected"}
              </span>
              {multiple && picked.length > 1 && (
                <button
                  onClick={() => setPicked([])}
                  disabled={busy}
                  className="text-[11px] font-medium text-content-muted transition-colors hover:text-content disabled:opacity-50"
                >
                  Clear
                </button>
              )}
            </div>
            <div className="mt-1.5 flex max-h-52 flex-col gap-1 overflow-y-auto">
              {picked.map((path) => (
                <div
                  key={path}
                  className="flex items-center gap-2.5 rounded-lg border border-border-soft bg-surface-2/50 px-3 py-2"
                >
                  <FolderOpen className="size-3.5 shrink-0 text-content-faint" />
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-[13px] text-content">
                      {baseName(path)}
                    </span>
                    <span className="block truncate font-mono text-[10px] text-content-faint">
                      {path}
                    </span>
                  </span>
                  <button
                    onClick={() => remove(path)}
                    disabled={busy}
                    aria-label={`Remove ${baseName(path)}`}
                    className="grid size-6 shrink-0 place-items-center rounded-md text-content-faint transition-colors hover:bg-surface-3 hover:text-content disabled:opacity-50"
                  >
                    <X className="size-3.5" />
                  </button>
                </div>
              ))}
            </div>
          </div>
        )}
      </div>

      <ModalFooter>
        <button
          onClick={onClose}
          disabled={busy}
          className="rounded-lg px-3 py-2 text-sm font-medium text-content-muted transition-colors hover:text-content disabled:opacity-50"
        >
          Cancel
        </button>
        <button
          onClick={() => onConfirm(picked)}
          disabled={picked.length === 0 || busy}
          className="inline-flex items-center gap-1.5 rounded-lg px-4 py-2 text-sm font-semibold text-black shadow-md shadow-(color:--accent-glow) transition-all [background:linear-gradient(to_bottom,var(--accent),var(--accent-deep))] hover:[background:linear-gradient(to_bottom,var(--accent-bright),var(--accent))] disabled:cursor-not-allowed disabled:opacity-45"
        >
          {busy && <Loader2 className="size-3.5 animate-spin" />}
          {confirmLabel}
        </button>
      </ModalFooter>
    </Modal>
  );
}
