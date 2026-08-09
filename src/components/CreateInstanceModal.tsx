import { useEffect, useMemo, useState } from "react";
import { Check, ChevronRight, FileArchive, Link2, Loader2, Package, Search, Wrench } from "lucide-react";

import { cn } from "../lib/cn";
import { api } from "../lib/api";
import { LOADERS } from "../lib/loader";
import { pickPackwizFile, type PackImportSource } from "../lib/packs";
import { Modal, ModalHeader } from "./Modal";
import { Select } from "./Select";
import type { LoaderKind, VersionEntry } from "../lib/types";
import { useStore } from "../store";

type Mode = "blank" | "packwiz" | null;

function Choice({
  icon: Icon,
  title,
  description,
  onClick,
}: {
  icon: typeof Package;
  title: string;
  description: string;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      className="group flex w-full items-center gap-3.5 rounded-xl border border-border-soft bg-surface-2/50 px-4 py-3.5 text-left transition-colors hover:border-border hover:bg-surface-2"
    >
      <span className="grid size-10 shrink-0 place-items-center rounded-xl border border-border-soft bg-surface-3 text-content-muted transition-colors group-hover:text-(--accent)">
        <Icon className="size-4.5" />
      </span>
      <span className="min-w-0 flex-1">
        <span className="block text-sm font-medium text-content">{title}</span>
        <span className="mt-0.5 block text-[11px] leading-relaxed text-content-muted">
          {description}
        </span>
      </span>
      <ChevronRight className="size-4 shrink-0 text-content-faint transition-transform group-hover:translate-x-0.5" />
    </button>
  );
}

export function CreateInstanceModal({
  open,
  onClose,
  onCreated,
  onImportFile,
  onImportPackwiz,
}: {
  open: boolean;
  onClose: () => void;
  onCreated: (id: string) => void;
  onImportFile?: () => void;
  onImportPackwiz?: (source: PackImportSource) => void;
}) {
  const createInstance = useStore((s) => s.createInstance);

  const [mode, setMode] = useState<Mode>(null);
  const [packwizUrl, setPackwizUrl] = useState("");

  const [versions, setVersions] = useState<VersionEntry[]>([]);
  const [includeSnapshots, setIncludeSnapshots] = useState(false);
  const [loading, setLoading] = useState(false);
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<string | null>(null);
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);
  const [loader, setLoader] = useState<LoaderKind | null>(null);
  const [loaderVersions, setLoaderVersions] = useState<string[]>([]);
  const [loaderVersion, setLoaderVersion] = useState<string | null>(null);
  const [loaderLoading, setLoaderLoading] = useState(false);

  useEffect(() => {
    if (open) {
      setMode(null);
      setPackwizUrl("");
    }
  }, [open]);

  useEffect(() => {
    if (!open || mode !== "blank") return;
    setLoading(true);
    api
      .listVersions(includeSnapshots)
      .then((v) => setVersions(v))
      .finally(() => setLoading(false));
  }, [open, mode, includeSnapshots]);

  useEffect(() => {
    setLoaderVersions([]);
    setLoaderVersion(null);
    if (!loader || !selected) return;
    let live = true;
    setLoaderLoading(true);
    api
      .listLoaderVersions(loader, selected)
      .then((list) => {
        if (!live) return;
        setLoaderVersions(list);
        setLoaderVersion(list[0] ?? null);
      })
      .catch(() => live && setLoaderVersions([]))
      .finally(() => live && setLoaderLoading(false));
    return () => {
      live = false;
    };
  }, [loader, selected]);

  const filtered = useMemo(
    () => versions.filter((v) => v.id.toLowerCase().includes(query.toLowerCase())),
    [versions, query],
  );

  const createDisabled = !selected || busy || (loader !== null && !loaderVersion);
  const packwizSource = packwizUrl.trim();
  const validPackwizUrl = /^https?:\/\//i.test(packwizSource);

  const importPackwizUrl = () => {
    if (!validPackwizUrl) return;
    onImportPackwiz?.({ kind: "url", value: packwizSource });
    onClose();
  };

  const create = async () => {
    if (!selected || (loader && !loaderVersion)) return;
    setBusy(true);
    try {
      const instance = await createInstance(
        name.trim() || selected,
        selected,
        loader,
        loader ? loaderVersion : null,
      );
      onCreated(instance.id);
      onClose();
      setSelected(null);
      setName("");
      setQuery("");
      setLoader(null);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal open={open} onClose={onClose} size="lg" labelledBy="new-instance-title">
      <ModalHeader
        id="new-instance-title"
        title="New instance"
        subtitle={
          mode === "blank"
            ? "Pick a game version and a loader"
            : mode === "packwiz"
              ? "Choose a local pack or enter its hosted URL"
              : "Where should it come from?"
        }
        onBack={mode === null ? undefined : () => setMode(null)}
        onClose={onClose}
      />

      {mode === null && (
        <div className="flex flex-col gap-2.5 px-5 py-5">
          <Choice
            icon={Package}
            title="Browse modpacks"
            description="Search Modrinth and CurseForge, then install a pack with everything set up."
            onClick={() => {
              onClose();
              useStore.getState().openDiscover("modpacks", null);
            }}
          />
          {onImportFile && (
            <Choice
              icon={FileArchive}
              title="Import a pack file"
              description="Open an .mrpack, CurseForge zip, or local pack.toml."
              onClick={() => {
                onClose();
                onImportFile();
              }}
            />
          )}
          {onImportPackwiz && (
            <Choice
              icon={Link2}
              title="Install from packwiz"
              description="Use a hosted pack.toml URL or choose a local packwiz pack."
              onClick={() => setMode("packwiz")}
            />
          )}
          <Choice
            icon={Wrench}
            title="Start from nothing"
            description="Choose a Minecraft version and a mod loader, then add content yourself."
            onClick={() => setMode("blank")}
          />
        </div>
      )}

      {mode === "packwiz" && (
        <div className="flex flex-col gap-4 px-5 py-5">
          <button
            onClick={async () => {
              const source = await pickPackwizFile();
              if (!source) return;
              onImportPackwiz?.({ kind: "file", value: source });
              onClose();
            }}
            className="flex items-center gap-3 rounded-xl border border-border-soft bg-surface-2/50 px-4 py-3.5 text-left transition-colors hover:border-border hover:bg-surface-2"
          >
            <span className="grid size-10 shrink-0 place-items-center rounded-xl border border-border-soft bg-surface-3 text-content-muted">
              <FileArchive className="size-4.5" />
            </span>
            <span>
              <span className="block text-sm font-medium text-content">Choose local pack.toml</span>
              <span className="mt-0.5 block text-[11px] text-content-muted">
                Basalt resolves the index and bundled files from the same folder.
              </span>
            </span>
          </button>

          <div className="flex items-center gap-3 text-[10px] font-semibold uppercase tracking-wide text-content-faint">
            <span className="h-px flex-1 bg-border-soft" />
            or use a URL
            <span className="h-px flex-1 bg-border-soft" />
          </div>

          <div className="flex flex-col gap-1.5">
            <div className="flex gap-2">
              <input
                value={packwizUrl}
                onChange={(event) => setPackwizUrl(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter") importPackwizUrl();
                }}
                placeholder="https://example.org/modpack/pack.toml"
                className="min-w-0 flex-1 rounded-lg border border-border bg-void px-3 py-2.5 text-sm text-content outline-none transition-colors focus:border-(--accent)"
              />
              <button
                onClick={importPackwizUrl}
                disabled={!validPackwizUrl}
                className="rounded-lg bg-(--accent) px-4 py-2.5 text-sm font-semibold text-black transition-opacity disabled:cursor-not-allowed disabled:opacity-45"
              >
                Continue
              </button>
            </div>
            {packwizSource !== "" && !validPackwizUrl && (
              <p className="text-[11px] text-warn">
                Paste the full address, starting with https://
              </p>
            )}
          </div>
        </div>
      )}

      {mode === "blank" && (
        <>

            <div className="flex flex-col gap-3 px-5 py-4">
              <input
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="Instance name (optional)"
                className="w-full rounded-lg border border-border bg-void px-3 py-2.5 text-sm text-content outline-none focus:border-lava"
              />

              <div className="flex items-center gap-2">
                <div className="relative flex-1">
                  <Search className="absolute left-3 top-1/2 size-4 -translate-y-1/2 text-content-faint" />
                  <input
                    value={query}
                    onChange={(e) => setQuery(e.target.value)}
                    placeholder="Search versions"
                    className="w-full rounded-lg border border-border bg-void py-2.5 pl-9 pr-3 text-sm text-content outline-none focus:border-lava"
                  />
                </div>
                <button
                  onClick={() => setIncludeSnapshots((v) => !v)}
                  className={cn(
                    "shrink-0 rounded-lg border px-3 py-2.5 text-xs font-medium transition-colors",
                    includeSnapshots
                      ? "border-lava/50 bg-lava/10 text-ember"
                      : "border-border bg-surface-2 text-content-muted hover:text-content",
                  )}
                >
                  Snapshots
                </button>
              </div>

              <div className="flex items-center gap-2">
                <div className="flex flex-1 rounded-lg border border-border bg-surface-2 p-0.5">
                  <button
                    onClick={() => setLoader(null)}
                    className={cn(
                      "flex-1 rounded-md px-2 py-1.5 text-xs font-medium transition-colors",
                      loader === null
                        ? "bg-surface-3 text-content"
                        : "text-content-faint hover:text-content-muted",
                    )}
                  >
                    Vanilla
                  </button>
                  {LOADERS.map((l) => (
                    <button
                      key={l.id}
                      onClick={() => setLoader(l.id)}
                      className={cn(
                        "flex-1 rounded-md px-2 py-1.5 text-xs font-medium transition-colors",
                        loader === l.id
                          ? "bg-surface-3 text-content"
                          : "text-content-faint hover:text-content-muted",
                      )}
                    >
                      {l.label}
                    </button>
                  ))}
                </div>
              </div>

              {loader && (
                <div className="flex items-center gap-2">
                  {loaderLoading ? (
                    <div className="flex items-center gap-2 py-1 text-xs text-content-muted">
                      <Loader2 className="size-3.5 animate-spin" />
                      Loading loader versions
                    </div>
                  ) : !selected ? (
                    <div className="py-1 text-xs text-content-faint">
                      Pick a game version to list loader builds.
                    </div>
                  ) : loaderVersions.length === 0 ? (
                    <div className="py-1 text-xs text-warn">
                      No {loader} builds for {selected}.
                    </div>
                  ) : (
                    <Select
                      value={loaderVersion}
                      options={loaderVersions.slice(0, 100)}
                      onChange={setLoaderVersion}
                      placeholder="Loader version"
                    />
                  )}
                </div>
              )}
            </div>

            <div className="min-h-0 flex-1 overflow-y-auto border-t border-border-soft px-2 py-2">
              {loading ? (
                <div className="flex items-center justify-center gap-2 py-10 text-sm text-content-muted">
                  <Loader2 className="size-4 animate-spin" />
                  Loading versions
                </div>
              ) : (
                filtered.slice(0, 300).map((v) => (
                  <button
                    key={v.id}
                    onClick={() => setSelected(v.id)}
                    className={cn(
                      "flex w-full items-center justify-between rounded-lg px-3 py-2 text-left text-sm transition-colors",
                      selected === v.id
                        ? "bg-lava/10 text-content"
                        : "text-content-muted hover:bg-surface-2 hover:text-content",
                    )}
                  >
                    <span className="font-mono">{v.id}</span>
                    <span className="flex items-center gap-2">
                      <span className="text-[11px] uppercase tracking-wide text-content-faint">
                        {v.type}
                      </span>
                      {selected === v.id && <Check className="size-4 text-lava" />}
                    </span>
                  </button>
                ))
              )}
            </div>

            <div className="flex items-center justify-end gap-2 border-t border-border-soft px-5 py-4">
              <button
                onClick={onClose}
                className="rounded-lg border border-border bg-surface-2 px-4 py-2 text-sm font-medium text-content hover:bg-surface-3"
              >
                Cancel
              </button>
              <button
                onClick={create}
                disabled={createDisabled}
                className="inline-flex items-center gap-2 rounded-lg bg-linear-to-b from-lava to-lava-deep px-4 py-2 text-sm font-semibold text-black shadow-lg shadow-lava/20 transition-all hover:from-lava-bright hover:to-lava disabled:cursor-not-allowed disabled:opacity-50"
              >
                {busy && <Loader2 className="size-4 animate-spin" />}
                Create
              </button>
            </div>
        </>
      )}
    </Modal>
  );
}
