import { open as openFileDialog, save as saveFileDialog } from "@tauri-apps/plugin-dialog";

import type { PackFormat } from "./types";

export const PACK_FORMATS: Array<{
  id: PackFormat;
  label: string;
  extension: string;
  note: string;
}> = [
  {
    id: "mrpack",
    label: "Modrinth",
    extension: "mrpack",
    note: "Modrinth mods are listed by download link, everything else travels inside the file.",
  },
  {
    id: "curseforge",
    label: "CurseForge",
    extension: "zip",
    note: "CurseForge mods are listed by project, everything else travels inside the file.",
  },
];

export function formatBytes(bytes: number): string {
  if (bytes <= 0) return "0 KB";
  if (bytes < 1024 ** 2) return `${Math.max(1, Math.round(bytes / 1024))} KB`;
  if (bytes < 1024 ** 3) return `${(bytes / 1024 ** 2).toFixed(0)} MB`;
  return `${(bytes / 1024 ** 3).toFixed(1)} GB`;
}

export async function pickPackFile(): Promise<string | null> {
  const chosen = await openFileDialog({
    multiple: false,
    directory: false,
    title: "Choose a modpack file",
    filters: [{ name: "Modpack", extensions: ["mrpack", "zip"] }],
  });
  return typeof chosen === "string" ? chosen : null;
}

export async function pickBannerFile(mode: "banner" | "logo"): Promise<string | null> {
  const extensions =
    mode === "logo"
      ? ["png", "jpg", "jpeg", "webp", "gif"]
      : ["png", "jpg", "jpeg", "webp", "gif", "mp4", "webm", "mkv", "mov"];
  const chosen = await openFileDialog({
    multiple: false,
    directory: false,
    title: mode === "logo" ? "Choose a logo" : "Choose a banner",
    filters: [{ name: mode === "logo" ? "Images" : "Images and video", extensions }],
  });
  return typeof chosen === "string" ? chosen : null;
}

export async function pickPackDestination(
  suggested: string,
  format: PackFormat,
): Promise<string | null> {
  const extension = PACK_FORMATS.find((entry) => entry.id === format)?.extension ?? "zip";
  const chosen = await saveFileDialog({
    title: "Export modpack",
    defaultPath: suggested,
    filters: [{ name: "Modpack", extensions: [extension] }],
  });
  return typeof chosen === "string" ? chosen : null;
}
