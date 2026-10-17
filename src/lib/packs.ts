import { downloadDir, homeDir, join } from "@tauri-apps/api/path";
import { open as openFileDialog, save as saveFileDialog } from "@tauri-apps/plugin-dialog";

import type { PackFormat } from "./types";

export type PackImportSource =
  | { kind: "file"; value: string }
  | { kind: "url"; value: string };

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
    note: "Mods Modrinth knows go by link, everything else is packed inside.",
  },
  {
    id: "curseforge",
    label: "CurseForge",
    extension: "zip",
    note: "Mods CurseForge knows go by project id, everything else is packed inside.",
  },
];

/**
 * Dialogs start in the working directory, which inside an AppImage is the
 * mount point, so every picker gets an explicit place to open.
 */
async function startDir(): Promise<string> {
  try {
    return await downloadDir();
  } catch {
    return homeDir();
  }
}

export async function pickPackFile(): Promise<string | null> {
  const chosen = await openFileDialog({
    multiple: false,
    directory: false,
    defaultPath: await startDir(),
    title: "Choose a modpack file",
    filters: [{ name: "Modpack", extensions: ["mrpack", "zip", "toml"] }],
  });
  return typeof chosen === "string" ? chosen : null;
}

export async function pickPackwizFile(): Promise<string | null> {
  const chosen = await openFileDialog({
    multiple: false,
    directory: false,
    defaultPath: await startDir(),
    title: "Choose pack.toml",
    filters: [{ name: "packwiz pack", extensions: ["toml"] }],
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
    defaultPath: await homeDir(),
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
    defaultPath: await join(await startDir(), suggested),
    filters: [{ name: "Modpack", extensions: [extension] }],
  });
  return typeof chosen === "string" ? chosen : null;
}
