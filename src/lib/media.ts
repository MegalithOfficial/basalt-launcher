import { convertFileSrc } from "@tauri-apps/api/core";

import type { VersionMedia } from "./types";

export function mediaSrc(media: VersionMedia): string {
  return media.local ? convertFileSrc(media.image_url) : media.image_url;
}
