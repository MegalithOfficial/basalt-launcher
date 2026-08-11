import { useEffect, useMemo, useState } from "react";
import { open as openFolderDialog } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ChevronRight, FolderInput, Loader2, Search, Server as ServerIcon } from "lucide-react";

import { api } from "../lib/api";
import { cn } from "../lib/cn";
import { flavorLabel, isNative, needsFlavorVersion, softwareOf } from "../lib/servers";
import type { ServerFlavor, ServerFolder, VersionEntry } from "../lib/types";
import { Modal, ModalHeader } from "./Modal";
import { Select } from "./Select";
import { useStore } from "../store";

type Mode = "new" | "import" | null;

const EULA_URL = "https://aka.ms/MinecraftEULA";

function Choice({
  icon: Icon,
  title,
  description,
  onClick,
}: {
  icon: typeof ServerIcon;
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

export function CreateServerModal({
  open,
  onClose,
  onCreated,
}: {
  open: boolean;
  onClose: () => void;
  onCreated: (id: string) => void;
}) {
  const createServer = useStore((s) => s.createServer);
  const importServer = useStore((s) => s.importServer);

  const [mode, setMode] = useState<Mode>(null);
  const [name, setName] = useState("");
  const software = useStore((s) => s.serverSoftware);
  const [flavor, setFlavor] = useState<ServerFlavor>("paper");
  const [versions, setVersions] = useState<VersionEntry[]>([]);
  const [query, setQuery] = useState("");
  const [version, setVersion] = useState<string | null>(null);
  const [builds, setBuilds] = useState<string[]>([]);
  const [build, setBuild] = useState<string | null>(null);
  const [buildsLoading, setBuildsLoading] = useState(false);
  const [folder, setFolder] = useState<ServerFolder | null>(null);
  const [eula, setEula] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setMode(null);
    setName("");
    setQuery("");
    setFolder(null);
    setEula(false);
    setError(null);
  }, [open]);

  useEffect(() => {
    if (!open || mode !== "new") return;
    void api.listVersions(false).then((list) => {
      setVersions(list);
      setVersion((current) => current ?? list[0]?.id ?? null);
    });
  }, [open, mode]);

  useEffect(() => {
    setBuilds([]);
    setBuild(null);
    if (!version || !needsFlavorVersion(software, flavor) || mode !== "new") return;
    let live = true;
    setBuildsLoading(true);
    api
      .listServerFlavorVersions(flavor, version)
      .then((list) => {
        if (!live) return;
        setBuilds(list);
        setBuild(list[0] ?? null);
      })
      .catch(() => live && setBuilds([]))
      .finally(() => live && setBuildsLoading(false));
    return () => {
      live = false;
    };
  }, [flavor, version, mode]);

  const filtered = useMemo(
    () => versions.filter((entry) => entry.id.toLowerCase().includes(query.toLowerCase())),
    [versions, query],
  );

  const pickFolder = async () => {
    const picked = await openFolderDialog({ directory: true, multiple: false });
    if (typeof picked !== "string") return;
    setError(null);
    try {
      const inspected = await api.inspectServerFolder(picked);
      setFolder(inspected);
      setName((current) => current || inspected.name);
      if (inspected.flavor) setFlavor(inspected.flavor);
      if (inspected.version_id) setVersion(inspected.version_id);
      setBuild(inspected.flavor_version);
      setEula(inspected.eula_accepted);
    } catch (cause) {
      setFolder(null);
      setError(String(cause));
    }
  };

  const native = isNative(software, flavor);
  const missingBuild = needsFlavorVersion(software, flavor) && !build;
  const ready = native
    ? mode === "new"
      ? !busy
      : !!folder && !busy
    : mode === "new"
      ? !!version && !missingBuild && eula && !busy
      : !!folder && !!version && eula && !busy;

  const submit = async () => {
    if (!ready) return;
    const chosen = native ? "nightly" : version;
    if (!chosen) return;
    setBusy(true);
    setError(null);
    try {
      const server =
        mode === "import" && folder
          ? await importServer(folder.path, name.trim() || folder.name, flavor, chosen, build)
          : await createServer(name.trim() || `${flavor} ${chosen}`, flavor, chosen, build);
      onCreated(server.id);
      onClose();
    } catch (cause) {
      setError(String(cause));
    } finally {
      setBusy(false);
    }
  };

  const eulaCheckbox = (
    <label className="flex cursor-pointer items-start gap-2.5 rounded-xl border border-border-soft bg-surface-2/50 px-4 py-3">
      <input
        type="checkbox"
        checked={eula}
        onChange={(event) => setEula(event.target.checked)}
        className="mt-0.5 size-4 shrink-0 accent-(--accent)"
      />
      <span className="text-[11px] leading-relaxed text-content-muted">
        I agree to the{" "}
        <button
          onClick={(event) => {
            event.preventDefault();
            void openUrl(EULA_URL);
          }}
          className="text-(--accent) underline underline-offset-2"
        >
          Minecraft EULA
        </button>
        . Basalt writes eula.txt for you once you accept, and a server will not start without it.
      </span>
    </label>
  );

  return (
    <Modal open={open} onClose={onClose} size="lg">
      <ModalHeader
        title={mode === "import" ? "Import a server" : mode === "new" ? "New server" : "Add a server"}
        onClose={onClose}
        onBack={mode ? () => setMode(null) : undefined}
      />

      {mode === null && (
        <div className="flex flex-col gap-2.5 px-5 py-5">
          <Choice
            icon={ServerIcon}
            title="Create a new server"
            description="Pick a version and a flavour, Basalt downloads and sets it up."
            onClick={() => setMode("new")}
          />
          <Choice
            icon={FolderInput}
            title="Import an existing folder"
            description="Keep the folder where it is and manage it from Basalt."
            onClick={() => setMode("import")}
          />
        </div>
      )}

      {mode !== null && (
        <div className="flex flex-col gap-3 px-5 py-4">
          {mode === "import" && (
            <button
              onClick={() => void pickFolder()}
              className="flex items-center gap-3 rounded-xl border border-border-soft bg-surface-2/50 px-4 py-3 text-left transition-colors hover:border-border hover:bg-surface-2"
            >
              <span className="grid size-9 shrink-0 place-items-center rounded-lg border border-border-soft bg-surface-3 text-content-muted">
                <FolderInput className="size-4" />
              </span>
              <span className="min-w-0 flex-1">
                <span className="block truncate text-sm font-medium text-content">
                  {folder ? folder.path : "Choose the server folder"}
                </span>
                <span className="mt-0.5 block text-[11px] text-content-muted">
                  {folder
                    ? "Basalt reads this folder in place and never moves it."
                    : "The folder that holds server.properties and the server jar."}
                </span>
              </span>
            </button>
          )}

          <input
            value={name}
            onChange={(event) => setName(event.target.value)}
            placeholder="Server name"
            className="w-full rounded-lg border border-border bg-void px-3 py-2.5 text-sm text-content outline-none focus:border-(--accent)"
          />

          <div className="grid grid-cols-2 gap-2">
            <Select
              value={flavorLabel(software, flavor)}
              onChange={(label) => {
                const picked = software.find((entry) => entry.label === label);
                if (picked) setFlavor(picked.id);
              }}
              options={software.map((entry) => entry.label)}
            />
            {native ? (
              <div className="grid place-items-center rounded-lg border border-border-soft bg-surface-2/50 text-[11px] text-content-faint">
                Nightly build
              </div>
            ) : needsFlavorVersion(software, flavor) ? (
              <Select
                value={build}
                onChange={(value) => setBuild(value)}
                options={builds.slice(0, 60)}
                placeholder={buildsLoading ? "Loading" : "Nothing published"}
              />
            ) : (
              <div className="grid place-items-center rounded-lg border border-border-soft bg-surface-2/50 text-[11px] text-content-faint">
                No build to pick
              </div>
            )}
          </div>

          <p className="text-[11px] text-content-muted">
            {softwareOf(software, flavor)?.hint}
          </p>

          {native ? (
            <p className="rounded-xl border border-border-soft bg-surface-2/50 px-4 py-3 text-[11px] leading-relaxed text-content-muted">
              {softwareOf(software, flavor)?.label} ships one rolling nightly build, so there is no
              version to pick here. It needs no Java and no EULA, and its settings live in{" "}
              {softwareOf(software, flavor)?.config_file} rather than server.properties.
            </p>
          ) : mode === "new" ? (
            <>
              <div className="relative">
                <Search className="absolute left-3 top-1/2 size-4 -translate-y-1/2 text-content-faint" />
                <input
                  value={query}
                  onChange={(event) => setQuery(event.target.value)}
                  placeholder="Search versions"
                  className="w-full rounded-lg border border-border bg-void py-2.5 pl-9 pr-3 text-sm text-content outline-none focus:border-(--accent)"
                />
              </div>
              <div className="max-h-52 overflow-y-auto rounded-lg border border-border-soft">
                {filtered.slice(0, 120).map((entry) => (
                  <button
                    key={entry.id}
                    onClick={() => setVersion(entry.id)}
                    className={cn(
                      "flex w-full items-center justify-between px-3 py-2 text-left text-sm transition-colors",
                      version === entry.id
                        ? "bg-(--accent)/15 text-content"
                        : "text-content-muted hover:bg-surface-2",
                    )}
                  >
                    <span>{entry.id}</span>
                    {version === entry.id && (
                      <span className="text-[10px] uppercase tracking-wider text-(--accent)">
                        Picked
                      </span>
                    )}
                  </button>
                ))}
              </div>
            </>
          ) : (
            <input
              value={version ?? ""}
              onChange={(event) => setVersion(event.target.value || null)}
              placeholder="Minecraft version, for example 1.21.8"
              className="w-full rounded-lg border border-border bg-void px-3 py-2.5 text-sm text-content outline-none focus:border-(--accent)"
            />
          )}

          {!native && eulaCheckbox}

          {error && (
            <p className="wrap-break-word text-[11px] text-danger">{error}</p>
          )}

          <button
            onClick={() => void submit()}
            disabled={!ready}
            className="inline-flex items-center justify-center gap-2 rounded-lg bg-(--accent) px-4 py-2.5 text-sm font-semibold text-black transition-opacity disabled:cursor-not-allowed disabled:opacity-45"
          >
            {busy && <Loader2 className="size-4 animate-spin" />}
            {mode === "import" ? "Import server" : "Create server"}
          </button>
        </div>
      )}
    </Modal>
  );
}
