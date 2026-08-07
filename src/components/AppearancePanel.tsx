import { useCallback, useEffect, useRef, useState } from "react";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import {
  Check,
  Loader2,
  Pencil,
  RotateCcw,
  Search,
  Trash2,
  TriangleAlert,
  Upload,
  UserRoundX,
} from "lucide-react";

import { api } from "../lib/api";
import { notifyRemoved } from "../lib/notify";
import { ConfirmDialog } from "./ConfirmDialog";
import { cn } from "../lib/cn";
import { log } from "../lib/log";
import { useStore } from "../store";
import type { Appearance, SkinEntry, SkinVariant } from "../lib/types";
import { SkinViewer } from "./SkinViewer";
import { CAPE_FRONT, FACE, FACE_OVERLAY, TextureCrop } from "./TextureCrop";

const VARIANTS: Array<{ id: SkinVariant; label: string }> = [
  { id: "classic", label: "Classic" },
  { id: "slim", label: "Slim" },
];

function SectionLabel({ title, aside }: { title: string; aside?: React.ReactNode }) {
  return (
    <div className="mb-3 flex items-center gap-4">
      <h3 className="shrink-0 text-[11px] font-semibold uppercase tracking-wider text-content-faint">
        {title}
      </h3>
      <span className="h-px flex-1 bg-border-soft" />
      {aside}
    </div>
  );
}

function Shimmer({ className }: { className?: string }) {
  return <div className={cn("animate-pulse rounded-lg bg-surface-3/50", className)} />;
}

function CharacterShimmer() {
  return (
    <div className="flex size-full items-end justify-center p-8">
      <div className="flex w-40 flex-col items-center gap-2">
        <Shimmer className="size-16 rounded-xl" />
        <div className="flex gap-2">
          <Shimmer className="h-24 w-5" />
          <Shimmer className="h-24 w-12" />
          <Shimmer className="h-24 w-5" />
        </div>
        <div className="flex gap-2">
          <Shimmer className="h-20 w-5" />
          <Shimmer className="h-20 w-5" />
        </div>
      </div>
    </div>
  );
}

function TileShimmers({ count }: { count: number }) {
  return (
    <>
      {Array.from({ length: count }).map((_, i) => (
        <div
          key={i}
          className="flex flex-col items-center gap-2 rounded-xl border border-border-soft bg-surface-2/60 p-3"
        >
          <Shimmer className="size-14 rounded-lg" />
          <Shimmer className="h-3 w-14" />
          <Shimmer className="h-2.5 w-9" />
        </div>
      ))}
    </>
  );
}

function SkinTile({
  skin,
  selected,
  worn,
  onSelect,
  onDelete,
  onRename,
}: {
  skin: SkinEntry;
  selected: boolean;
  worn: boolean;
  onSelect: () => void;
  onDelete: () => void;
  onRename: (name: string) => void;
}) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(skin.name);

  const commit = () => {
    const next = draft.trim();
    setEditing(false);
    if (next && next !== skin.name) onRename(next);
    else setDraft(skin.name);
  };

  return (
    <div
      className={cn(
        "group relative flex min-w-0 flex-col items-center gap-2 rounded-xl border p-3 transition-colors",
        selected
          ? "border-(--accent)/50 bg-(--accent-glow)/25"
          : "border-border-soft bg-surface-2/60 hover:border-border",
      )}
    >
      <button
        onClick={onSelect}
        title={`Preview ${skin.name}`}
        className="flex w-full min-w-0 flex-col items-center gap-2"
      >
        <TextureCrop
          url={skin.data_url}
          crop={FACE}
          overlay={FACE_OVERLAY}
          className="size-14 rounded-lg"
        />
      </button>

      {editing ? (
        <input
          autoFocus
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onBlur={commit}
          onKeyDown={(e) => {
            if (e.key === "Enter") commit();
            if (e.key === "Escape") {
              setDraft(skin.name);
              setEditing(false);
            }
          }}
          className="w-full min-w-0 rounded border border-(--accent) bg-void px-1 py-0.5 text-center text-xs text-content outline-none"
        />
      ) : (
        <button
          onDoubleClick={() => setEditing(true)}
          onClick={onSelect}
          title={skin.name}
          className="w-full min-w-0 truncate text-center text-xs font-medium text-content"
        >
          {skin.name}
        </button>
      )}

      {worn ? (
        <span className="rounded border border-(--accent)/40 px-1.5 text-[10px] font-semibold uppercase tracking-wide text-(--accent-bright)">
          Worn
        </span>
      ) : (
        <span className="text-[10px] capitalize text-content-faint">{skin.variant}</span>
      )}

      <div className="absolute right-1.5 top-1.5 flex gap-0.5 opacity-0 transition-opacity focus-within:opacity-100 group-hover:opacity-100">
        <button
          onClick={() => setEditing(true)}
          title={`Rename ${skin.name}`}
          className="grid size-6 place-items-center rounded text-content-faint hover:bg-surface-3 hover:text-content"
        >
          <Pencil className="size-3.5" />
        </button>
        <button
          onClick={onDelete}
          title={`Remove ${skin.name}`}
          className="grid size-6 place-items-center rounded text-content-faint hover:bg-danger/15 hover:text-danger"
        >
          <Trash2 className="size-3.5" />
        </button>
      </div>
    </div>
  );
}

export function AppearancePanel({ accountName }: { accountName: string }) {
  const [appearance, setAppearance] = useState<Appearance | null>(null);
  const [skins, setSkins] = useState<SkinEntry[]>([]);
  const [variant, setVariant] = useState<SkinVariant>("classic");
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [player, setPlayer] = useState("");
  const [importing, setImporting] = useState(false);
  const [walking, setWalking] = useState(true);
  const [previewId, setPreviewId] = useState<string | null>(null);
  const [removingSkin, setRemovingSkin] = useState<SkinEntry | null>(null);
  const bumpSkinRevision = useStore((s) => s.bumpSkinRevision);
  const setSkinHead = useStore((s) => s.setSkinHead);

  const previewSkin = skins.find((s) => s.id === previewId) ?? null;
  const activeCape = appearance?.capes.find((c) => c.active) ?? null;

  useEffect(() => {
    if (!appearance) return;
    const worn = skins.find((s) => s.id === appearance.library_id);
    setSkinHead(appearance.uuid, worn?.data_url ?? null);
  }, [appearance, skins, setSkinHead]);

  const refreshSkins = useCallback(async () => {
    setSkins(await api.listSkins());
  }, []);

  const loaded = useRef(false);
  useEffect(() => {
    if (loaded.current) return;
    loaded.current = true;
    void (async () => {
      try {
        const [profile] = await Promise.all([api.getAppearance(), refreshSkins()]);
        setAppearance(profile);
        setVariant(profile.variant);
      } catch (e) {
        setError(String(e));
      } finally {
        setLoading(false);
      }
    })();
  }, [refreshSkins]);

  const run = async (task: () => Promise<Appearance>) => {
    setBusy(true);
    setError(null);
    try {
      const next = await task();
      setAppearance(next);
      setVariant(next.variant);
      setPreviewId(null);
      await refreshSkins();
      bumpSkinRevision();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const upload = async () => {
    const file = await openFileDialog({
      multiple: false,
      directory: false,
      filters: [{ name: "Skin", extensions: ["png"] }],
    });
    if (typeof file !== "string") return;
    setBusy(true);
    setError(null);
    try {
      const saved = await api.addSkinFromFile(file, null, variant);
      await refreshSkins();
      setPreviewId(saved.id);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const importPlayer = async () => {
    const name = player.trim();
    if (!name) return;
    setImporting(true);
    setError(null);
    try {
      const saved = await api.addSkinFromReference(name);
      await refreshSkins();
      setPreviewId(saved.id);
      setPlayer("");
    } catch (e) {
      setError(String(e));
    } finally {
      setImporting(false);
    }
  };

  const renameSkin = async (id: string, name: string) => {
    try {
      await api.renameSkin(id, name);
      await refreshSkins();
    } catch (e) {
      setError(String(e));
    }
  };

  const removeSkin = async (id: string) => {
    try {
      await api.deleteSkin(id);
      notifyRemoved("Deleted skin");
      if (previewId === id) setPreviewId(null);
      await refreshSkins();
    } catch (e) {
      log.warn("skins", `could not remove skin: ${String(e)}`);
    }
  };

  const shownSkin = previewSkin?.data_url ?? appearance?.skin_url ?? null;
  const shownSlim = (previewSkin?.variant ?? variant) === "slim";
  const wornId = appearance?.library_id ?? null;

  const chooseVariant = (next: SkinVariant) => {
    if (next === variant) return;
    setVariant(next);
    if (previewSkin || !wornId) return;
    void run(() => api.applySavedSkin(wornId, next));
  };

  return (
    <div className="flex flex-col gap-4">
      {error && (
        <div className="flex items-start gap-3 rounded-xl border border-danger/30 bg-danger/10 px-4 py-3 text-sm">
          <TriangleAlert className="mt-0.5 size-4 shrink-0 text-danger" />
          <span className="min-w-0 wrap-break-word text-content-muted">{error}</span>
        </div>
      )}

      <div className="flex flex-wrap items-stretch gap-5">
        <div className="relative flex w-88 shrink-0 flex-col overflow-hidden rounded-2xl border border-border-soft bg-surface-2/60">
          <div
            aria-hidden
            className="pointer-events-none absolute inset-0 [background:radial-gradient(80%_60%_at_50%_0%,var(--accent-glow),transparent_70%)]"
          />
          <div className="relative min-h-104 flex-1">
            {loading ? (
              <CharacterShimmer />
            ) : (
              <SkinViewer
                skinUrl={shownSkin}
                capeUrl={activeCape?.url ?? null}
                slim={shownSlim}
                walking={walking}
                className="h-full"
              />
            )}
            <button
              onClick={() => setWalking((v) => !v)}
              className="absolute right-3 top-3 rounded-md border border-border-soft bg-surface-2/80 px-2 py-1 text-[11px] font-medium text-content-faint backdrop-blur transition-colors hover:text-content"
            >
              {walking ? "Walking" : "Idle"}
            </button>
          </div>

          <div className="relative flex flex-col gap-2 border-t border-border-soft p-3">
            {previewSkin ? (
              <div className="flex items-center gap-2 rounded-lg border border-(--accent)/30 bg-(--accent-glow)/20 px-3 py-2">
                <span className="min-w-0 flex-1 truncate text-xs text-content-muted">
                  Previewing {previewSkin.name}
                </span>
                <button
                  onClick={() => setPreviewId(null)}
                  className="shrink-0 text-xs font-medium text-content-faint hover:text-content"
                >
                  Cancel
                </button>
                <button
                  onClick={() =>
                    run(() => api.applySavedSkin(previewSkin.id, previewSkin.variant))
                  }
                  disabled={busy}
                  className="inline-flex shrink-0 items-center gap-1.5 rounded-md px-2.5 py-1 text-[11px] font-semibold text-black [background:linear-gradient(to_bottom,var(--accent),var(--accent-deep))] disabled:opacity-50"
                >
                  {busy ? (
                    <Loader2 className="size-3 animate-spin" />
                  ) : (
                    <Check className="size-3" />
                  )}
                  Wear it
                </button>
              </div>
            ) : (
              <div
                className="flex items-center gap-0.5 rounded-lg border border-border-soft bg-surface-2/60 p-0.5"
                title={
                  wornId
                    ? "Changes the arm model on your account right away"
                    : "Used for the next skin you upload"
                }
              >
                {VARIANTS.map((option) => (
                  <button
                    key={option.id}
                    onClick={() => chooseVariant(option.id)}
                    disabled={busy}
                    className={cn(
                      "flex-1 rounded-md px-2 py-1.5 text-xs font-medium transition-colors",
                      variant === option.id
                        ? "bg-surface-3 text-content"
                        : "text-content-faint hover:text-content-muted",
                    )}
                  >
                    {option.label}
                  </button>
                ))}
              </div>
            )}

            <div className="flex gap-2">
              <button
                onClick={() => void upload()}
                disabled={busy}
                className="inline-flex flex-1 items-center justify-center gap-2 rounded-lg border border-border bg-surface-2 px-3 py-2 text-xs font-medium text-content transition-colors hover:bg-surface-3 disabled:opacity-50"
              >
                {busy ? (
                  <Loader2 className="size-3.5 animate-spin" />
                ) : (
                  <Upload className="size-3.5" />
                )}
                Upload PNG
              </button>
              <button
                onClick={() => void run(() => api.resetSkin())}
                disabled={busy || loading}
                title="Go back to the default skin"
                className="inline-flex items-center justify-center gap-2 rounded-lg border border-border bg-surface-2 px-3 py-2 text-xs font-medium text-content-muted transition-colors hover:bg-surface-3 hover:text-content disabled:opacity-50"
              >
                <RotateCcw className="size-3.5" />
                Reset
              </button>
            </div>
          </div>
        </div>

        <div className="flex min-w-88 flex-1 flex-col gap-6">
          <section>
            <SectionLabel
              title="Skin library"
              aside={
                <span className="shrink-0 text-[11px] text-content-faint">
                  {skins.length} saved
                </span>
              }
            />

            <div className="mb-3 flex flex-wrap items-center gap-2">
              <div className="relative min-w-56 flex-1">
                <Search className="pointer-events-none absolute left-3 top-1/2 size-3.5 -translate-y-1/2 text-content-faint" />
                <input
                  value={player}
                  onChange={(e) => setPlayer(e.target.value)}
                  onKeyDown={(e) => e.key === "Enter" && void importPlayer()}
                  placeholder="Player name, UUID, texture link, NameMC link or a /give command"
                  className="w-full rounded-lg border border-border bg-void py-2 pl-9 pr-3 text-sm text-content outline-none transition-colors placeholder:text-content-faint focus:border-(--accent)"
                />
              </div>
              <button
                onClick={() => void importPlayer()}
                disabled={importing || !player.trim()}
                className="inline-flex items-center gap-2 rounded-lg border border-border bg-surface-2 px-3 py-2 text-xs font-medium text-content transition-colors hover:bg-surface-3 disabled:cursor-not-allowed disabled:opacity-50"
              >
                {importing ? (
                  <Loader2 className="size-3.5 animate-spin" />
                ) : (
                  <Search className="size-3.5" />
                )}
                Import
              </button>
            </div>

            {loading && skins.length === 0 ? (
              <div className="grid gap-2 grid-cols-[repeat(auto-fill,minmax(6.5rem,1fr))]">
                <TileShimmers count={6} />
              </div>
            ) : skins.length === 0 ? (
              <div className="rounded-xl border border-dashed border-border px-4 py-6 text-sm text-content-faint">
Your current skin is saved here automatically. Upload a PNG or pull one in by
                player name, then click a face to try it on.
              </div>
            ) : (
              <div className="grid gap-2 grid-cols-[repeat(auto-fill,minmax(6.5rem,1fr))]">
                {skins.map((skin) => (
                  <SkinTile
                    key={skin.id}
                    skin={skin}
                    selected={previewId === skin.id}
                    worn={appearance?.library_id === skin.id}
                    onSelect={() => setPreviewId(skin.id)}
                    onDelete={() => setRemovingSkin(skin)}
                    onRename={(name) => void renameSkin(skin.id, name)}
                  />
                ))}
              </div>
            )}
          </section>

          <section>
            <SectionLabel title="Capes" />
            {loading ? (
              <div className="flex flex-wrap gap-2">
                {Array.from({ length: 4 }).map((_, i) => (
                  <div
                    key={i}
                    className="flex w-18 flex-col items-center gap-2 rounded-xl border border-border-soft bg-surface-2/60 p-2"
                  >
                    <Shimmer className="h-14 w-[2.2rem] rounded" />
                    <Shimmer className="h-2.5 w-10" />
                  </div>
                ))}
              </div>
            ) : appearance && appearance.capes.length > 0 ? (
              <div className="flex flex-wrap gap-2">
                <button
                  onClick={() => void run(() => api.setCape(null))}
                  disabled={busy}
                  className={cn(
                    "flex w-18 flex-col items-center gap-2 rounded-xl border p-2 transition-colors disabled:opacity-50",
                    !appearance.active_cape_id
                      ? "border-(--accent)/50 bg-(--accent-glow)/25"
                      : "border-border-soft bg-surface-2/60 hover:border-border",
                  )}
                >
                  <span className="grid h-14 w-[2.2rem] place-items-center rounded bg-void/50 text-content-faint">
                    <UserRoundX className="size-4" />
                  </span>
                  <span className="text-[10px] text-content-muted">None</span>
                </button>
                {appearance.capes.map((cape) => (
                  <button
                    key={cape.id}
                    onClick={() => void run(() => api.setCape(cape.id))}
                    disabled={busy}
                    title={cape.alias}
                    className={cn(
                      "flex w-18 flex-col items-center gap-2 rounded-xl border p-2 transition-colors disabled:opacity-50",
                      cape.active
                        ? "border-(--accent)/50 bg-(--accent-glow)/25"
                        : "border-border-soft bg-surface-2/60 hover:border-border",
                    )}
                  >
                    <TextureCrop
                      url={cape.url}
                      crop={CAPE_FRONT}
                      className="h-14 w-[2.2rem] rounded"
                    />
                    <span className="w-full truncate text-center text-[10px] text-content-muted">
                      {cape.alias}
                    </span>
                  </button>
                ))}
              </div>
            ) : (
              <div className="text-sm text-content-faint">
                {accountName} has no capes on this account.
              </div>
            )}
          </section>
        </div>
      </div>

      <ConfirmDialog
        open={!!removingSkin}
        title={removingSkin ? `Delete ${removingSkin.name}?` : ""}
        description="The skin file is removed from the library. Anyone currently wearing it keeps it until they change."
        confirmLabel="Delete skin"
        onConfirm={async () => {
          if (removingSkin) await removeSkin(removingSkin.id);
          setRemovingSkin(null);
        }}
        onCancel={() => setRemovingSkin(null)}
      />
    </div>
  );
}
