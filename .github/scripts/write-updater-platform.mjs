import { mkdirSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

const [, , profile = "", platform = ""] = process.argv;
if (!/^(debug|release)$/.test(profile)) {
  throw new Error(`Profile must be debug or release, received "${profile}"`);
}
if (!/^(linux|windows|darwin)-(x86_64|aarch64|i686|armv7)$/.test(platform)) {
  throw new Error(`Invalid updater platform "${platform}"`);
}

const bundle = resolve("src-tauri", "target", profile, "bundle");
mkdirSync(bundle, { recursive: true });
writeFileSync(resolve(bundle, "updater-platform.txt"), `${platform}\n`);
