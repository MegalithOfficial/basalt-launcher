import { useState } from "react";
import { Server as ServerIcon } from "lucide-react";
import { toast } from "sonner";

import { api } from "../lib/api";
import { formatBytes } from "../lib/format";
import type { ManualDownload, ProjectVersion, VersionFile } from "../lib/types";
import { Modal } from "./Modal";
import { useCurseforgeDownloads } from "./CurseForgeDownloadModal";
import { useStore } from "../store";

export function GetServerModal({
  open,
  title,
  version,
  file,
  fileId,
  projectId,
  onClose,
}: {
  open: boolean;
  title: string;
  version: ProjectVersion | null;
  file: VersionFile | null;
  fileId: string | null;
  projectId: string;
  onClose: () => void;
}) {
  const refreshServers = useStore((s) => s.refreshServers);
  const openServer = useStore((s) => s.openServer);
  const [name, setName] = useState("");
  const browserDownloads = useCurseforgeDownloads();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const install = async () => {
    if (!file) return;
    setBusy(true);
    setError(null);
    try {
      let localPath: string | null = null;
      if (!file.url) {
        const requirement: ManualDownload = {
          project_id: projectId,
          file_id: fileId ?? "",
          file_name: file.file_name,
          download_page_url: `https://www.curseforge.com/minecraft/modpacks/${projectId}/files/${fileId ?? ""}`,
          sha1: file.sha1,
          size: file.size,
          instance_path: file.file_name,
          pack_archive: true,
        };
        const collected = await browserDownloads.collect([requirement]);
        if (!collected?.length) return;
        localPath = collected[0].path;
      }
      const serverName = name.trim() || `${title} server`;
      onClose();
      const created = await api.installServerZip(
        serverName,
        file.url,
        localPath,
        file.file_name,
        file.sha1,
        file.size,
      );
      await refreshServers();
      toast.success(`${created.name} is ready`, {
        action: { label: "Open", onClick: () => openServer(created.id) },
      });
    } catch (cause) {
      toast.error(`Could not create ${title} server`, { description: String(cause) });
      setError(String(cause));
    } finally {
      setBusy(false);
    }
  };

  if (!open || !version || !file) return null;

  return (
    <Modal open onClose={onClose}>
      {browserDownloads.modal}
      <div className="border-b border-border-soft px-5 py-4">
        <h2 className="font-display text-[1rem] font-semibold text-content">Get the server</h2>
        <div className="mt-0.5 truncate text-xs text-content-muted">
          {title} {version.version_number}
        </div>
      </div>

      <div className="flex flex-col gap-3 px-5 py-4">
        <div className="rounded-xl border border-border-soft bg-surface-2/60 px-4 py-3">
          <div className="truncate font-mono text-[11px] text-content-muted">
            {file.file_name}
          </div>
          <div className="mt-1 text-[11px] text-content-faint">
            {formatBytes(file.size ?? 0)}
            {version.game_versions[0] && ` · ${version.game_versions[0]}`}
            {version.loaders[0] && ` · ${version.loaders[0]}`}
          </div>
        </div>

        <input
          value={name}
          onChange={(event) => setName(event.target.value)}
          placeholder={`${title} server`}
          className="w-full rounded-lg border border-border bg-void px-3 py-2.5 text-sm text-content outline-none focus:border-(--accent)"
        />

        <p className="text-[11px] leading-relaxed text-content-muted">
          {file.url
            ? "Basalt unpacks this pack into a new server folder and reads what it finds to work out the software and version."
            : "This author does not allow Basalt to download the file, so it opens in your browser and Basalt picks it up from your downloads folder."}
        </p>

        {error && <p className="wrap-break-word text-[11px] text-danger">{error}</p>}

        <button
          onClick={() => void install()}
          disabled={busy}
          className="inline-flex items-center justify-center gap-2 rounded-lg px-4 py-2.5 text-sm font-semibold text-black shadow-md shadow-(color:--accent-glow) transition-all [background:linear-gradient(to_bottom,var(--accent),var(--accent-deep))] hover:[background:linear-gradient(to_bottom,var(--accent-bright),var(--accent))] disabled:cursor-not-allowed disabled:opacity-40"
        >
          <ServerIcon className="size-4" />
          Create server
        </button>
      </div>
    </Modal>
  );
}
