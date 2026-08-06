import { useCallback, useEffect, useState } from "react";
import { Check, Film, ImagePlus, Images, Loader2, Trash2, TriangleAlert } from "lucide-react";

import { api } from "../lib/api";
import { cn } from "../lib/cn";
import { formatBytes } from "../lib/format";
import { pickBannerFile } from "../lib/packs";
import { relativeTime } from "../lib/time";
import type { BannerEntry } from "../lib/types";
import { Banner } from "./Banner";
import { ConfirmDialog } from "./ConfirmDialog";
import { Modal, ModalBody, ModalFooter, ModalHeader } from "./Modal";

export function BannerLibraryModal({
  open,
  mode,
  currentId,
  onClose,
  onPick,
}: {
  open: boolean;
  mode: "banner" | "logo";
  currentId?: string | null;
  onClose: () => void;
  onPick: (entry: BannerEntry) => Promise<void> | void;
}) {
  const [entries, setEntries] = useState<BannerEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [removing, setRemoving] = useState<BannerEntry | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setEntries(await api.listBannerLibrary());
    } catch (cause) {
      setError(String(cause));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (open) void load();
  }, [open, load]);

  const shown = mode === "logo" ? entries.filter((entry) => entry.kind === "image") : entries;

  const upload = async () => {
    const chosen = await pickBannerFile(mode);
    if (!chosen) return;
    setBusy("upload");
    setError(null);
    try {
      const added = await api.addBannerToLibrary(chosen);
      await load();
      await onPick(added);
      onClose();
    } catch (cause) {
      setError(String(cause));
    } finally {
      setBusy(null);
    }
  };

  const choose = async (entry: BannerEntry) => {
    setBusy(entry.id);
    setError(null);
    try {
      await onPick(entry);
      onClose();
    } catch (cause) {
      setError(String(cause));
    } finally {
      setBusy(null);
    }
  };

  const remove = async () => {
    if (!removing) return;
    await api.deleteBanner(removing.id);
    setRemoving(null);
    await load();
  };

  return (
    <>
      <Modal
        open={open}
        onClose={onClose}
        size="wide"
        nested
        className="h-[min(640px,calc(100vh-48px))]"
        labelledBy="banner-library-title"
      >
        <ModalHeader
          id="banner-library-title"
          title={mode === "logo" ? "Choose a logo" : "Choose a banner"}
          subtitle="Everything you have uploaded stays here, so you never have to find the file twice."
          icon={
            <div className="grid size-9 place-items-center rounded-xl border border-border-soft bg-surface-2 text-(--accent)">
              <Images className="size-4" />
            </div>
          }
          onClose={onClose}
        />

        <ModalBody>
          {loading ? (
            <div className="flex h-full items-center justify-center gap-2 text-sm text-content-muted">
              <Loader2 className="size-4 animate-spin" />
              Reading the library
            </div>
          ) : shown.length === 0 ? (
            <div className="flex h-full flex-col items-center justify-center gap-3 text-center">
              <div className="grid size-12 place-items-center rounded-2xl border border-border-soft bg-surface-2 text-content-faint">
                <Images className="size-6" />
              </div>
              <div className="text-sm font-medium text-content-muted">Nothing uploaded yet</div>
              <p className="max-w-sm text-xs text-content-faint">
                Add an image or video and it stays in the library for every instance.
              </p>
            </div>
          ) : (
            <div className="grid grid-cols-2 gap-3 sm:grid-cols-3">
              {shown.map((entry) => {
                const active = entry.id === currentId;
                return (
                  <div key={entry.id} className="group relative">
                    <button
                      onClick={() => void choose(entry)}
                      disabled={busy !== null}
                      className={cn(
                        "block w-full overflow-hidden rounded-xl border transition-colors",
                        active
                          ? "border-(--accent)"
                          : "border-border-soft hover:border-content-faint/50",
                        busy !== null && "cursor-wait",
                      )}
                    >
                      <span className="relative block aspect-16/10 bg-surface-3">
                        <Banner
                          media={{
                            image_url: entry.path,
                            short_text: null,
                            accent: entry.accent,
                            local: true,
                            kind: entry.kind,
                          }}
                          still
                          className="absolute inset-0 h-full w-full"
                        />
                        {entry.kind === "video" && (
                          <span className="absolute left-1.5 top-1.5 inline-flex items-center gap-1 rounded bg-black/70 px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-wide text-white/80 backdrop-blur">
                            <Film className="size-3" />
                            video
                          </span>
                        )}
                        {busy === entry.id && (
                          <span className="absolute inset-0 grid place-items-center bg-black/50">
                            <Loader2 className="size-5 animate-spin text-white" />
                          </span>
                        )}
                        {active && busy !== entry.id && (
                          <span className="absolute right-1.5 top-1.5 grid size-5 place-items-center rounded-full bg-(--accent) text-black">
                            <Check className="size-3" strokeWidth={4} />
                          </span>
                        )}
                      </span>
                      <span className="block px-2.5 py-2 text-left">
                        <span className="block truncate text-[11px] font-medium text-content">
                          {entry.original_name ?? entry.id.slice(0, 12)}
                        </span>
                        <span className="block truncate text-[10px] text-content-faint">
                          {formatBytes(entry.bytes)}
                          {entry.width && entry.height
                            ? ` · ${entry.width}x${entry.height}`
                            : ""}
                          {` · ${relativeTime(entry.added_at)}`}
                        </span>
                      </span>
                    </button>

                    <button
                      onClick={() => setRemoving(entry)}
                      aria-label="Delete from the library"
                      title="Delete from the library"
                      className="absolute right-1.5 bottom-11 grid size-7 place-items-center rounded-lg bg-black/60 text-white/70 opacity-0 backdrop-blur transition-opacity hover:bg-danger hover:text-white group-hover:opacity-100"
                    >
                      <Trash2 className="size-3.5" />
                    </button>
                  </div>
                );
              })}
            </div>
          )}

          {error && (
            <div className="mt-4 flex gap-2.5 rounded-xl border border-danger/25 bg-danger/[0.07] px-3.5 py-3 text-xs text-danger">
              <TriangleAlert className="mt-0.5 size-4 shrink-0" />
              <span className="break-words">{error}</span>
            </div>
          )}
        </ModalBody>

        <ModalFooter className="justify-between">
          <span className="text-[11px] text-content-faint">
            {mode === "logo"
              ? "Images only. Videos cannot be used as a logo."
              : "Images and videos up to 100 MB."}
          </span>
          <div className="flex items-center gap-2">
            <button
              onClick={onClose}
              className="rounded-lg px-3 py-2 text-sm font-medium text-content-muted transition-colors hover:text-content"
            >
              Cancel
            </button>
            <button
              onClick={() => void upload()}
              disabled={busy !== null}
              className="inline-flex items-center gap-1.5 rounded-lg px-4 py-2 text-sm font-semibold text-black shadow-md shadow-(color:--accent-glow) transition-all [background:linear-gradient(to_bottom,var(--accent),var(--accent-deep))] hover:[background:linear-gradient(to_bottom,var(--accent-bright),var(--accent))] disabled:cursor-not-allowed disabled:opacity-45"
            >
              {busy === "upload" ? (
                <Loader2 className="size-3.5 animate-spin" />
              ) : (
                <ImagePlus className="size-3.5" />
              )}
              Upload new
            </button>
          </div>
        </ModalFooter>
      </Modal>

      <ConfirmDialog
        open={!!removing}
        nested
        tone="danger"
        title={`Delete ${removing?.original_name ?? "this image"}?`}
        description={
          removing && removing.in_use_by.length > 0
            ? `${removing.in_use_by.join(", ")} ${
                removing.in_use_by.length === 1 ? "uses" : "use"
              } it and will fall back to the version artwork.`
            : "It leaves the library and the file is removed from disk."
        }
        confirmLabel="Delete"
        cancelLabel="Keep it"
        onConfirm={remove}
        onCancel={() => setRemoving(null)}
      />
    </>
  );
}
