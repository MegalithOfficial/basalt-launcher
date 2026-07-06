import { useEffect, useMemo, useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { Check, Loader2, Search, X } from "lucide-react";

import { cn } from "../lib/cn";
import { api } from "../lib/api";
import { LOADERS } from "../lib/loader";
import { useEscape } from "../lib/useEscape";
import { Select } from "./Select";
import type { LoaderKind, VersionEntry } from "../lib/types";
import { useStore } from "../store";

export function CreateInstanceModal({
  open,
  onClose,
  onCreated,
}: {
  open: boolean;
  onClose: () => void;
  onCreated: (id: string) => void;
}) {
  const createInstance = useStore((s) => s.createInstance);

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
    if (!open) return;
    setLoading(true);
    api
      .listVersions(includeSnapshots)
      .then((v) => setVersions(v))
      .finally(() => setLoading(false));
  }, [open, includeSnapshots]);

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

  useEscape(open, onClose);

  const createDisabled = !selected || busy || (loader !== null && !loaderVersion);

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
    <AnimatePresence>
      {open && (
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          className="fixed inset-0 z-50 grid place-items-center bg-black/60 p-6 backdrop-blur-sm"
          onClick={onClose}
        >
          <motion.div
            initial={{ opacity: 0, scale: 0.97, y: 8 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.97, y: 8 }}
            transition={{ duration: 0.18 }}
            onClick={(e) => e.stopPropagation()}
            className="flex max-h-[78vh] w-full max-w-lg flex-col overflow-hidden rounded-2xl border border-border bg-surface shadow-2xl"
          >
            <div className="flex items-center justify-between border-b border-border-soft px-5 py-4">
              <h2 className="font-display text-lg font-semibold text-content">New instance</h2>
              <button
                onClick={onClose}
                className="grid size-7 place-items-center rounded-md text-content-faint hover:bg-surface-2 hover:text-content"
              >
                <X className="size-4" />
              </button>
            </div>

            <div className="flex flex-col gap-3 px-5 py-4">
              <input
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="Instance name (optional)"
                className="w-full rounded-lg border border-border bg-base px-3 py-2.5 text-sm text-content outline-none focus:border-lava"
              />

              <div className="flex items-center gap-2">
                <div className="relative flex-1">
                  <Search className="absolute left-3 top-1/2 size-4 -translate-y-1/2 text-content-faint" />
                  <input
                    value={query}
                    onChange={(e) => setQuery(e.target.value)}
                    placeholder="Search versions"
                    className="w-full rounded-lg border border-border bg-base py-2.5 pl-9 pr-3 text-sm text-content outline-none focus:border-lava"
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

            <div className="flex justify-end gap-2 border-t border-border-soft px-5 py-4">
              <button
                onClick={onClose}
                className="rounded-lg border border-border bg-surface-2 px-4 py-2 text-sm font-medium text-content hover:bg-surface-3"
              >
                Cancel
              </button>
              <button
                onClick={create}
                disabled={createDisabled}
                className="inline-flex items-center gap-2 rounded-lg bg-gradient-to-b from-lava to-lava-deep px-4 py-2 text-sm font-semibold text-black shadow-lg shadow-lava/20 transition-all hover:from-lava-bright hover:to-lava disabled:cursor-not-allowed disabled:opacity-50"
              >
                {busy && <Loader2 className="size-4 animate-spin" />}
                Create
              </button>
            </div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
