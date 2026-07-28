import { convertFileSrc } from "@tauri-apps/api/core";

import type { VersionMedia } from "./types";

export function mediaSrc(media: VersionMedia): string {
  return media.local ? convertFileSrc(media.image_url) : media.image_url;
}

export function logoSrc(logo: string | null): string | null {
  if (!logo) return null;
  return /^https?:\/\//.test(logo) ? logo : convertFileSrc(logo);
}
