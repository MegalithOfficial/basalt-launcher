import { appendFileSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

const root = process.cwd();
const manifestPath = resolve(root, "src-tauri", "Cargo.toml");
const lockPath = resolve(root, "src-tauri", "Cargo.lock");
const requested = (process.argv[2] ?? "").trim().replace(/^v/, "");
const versionPattern = /^\d+\.\d+\.\d+$/;

let manifest = readFileSync(manifestPath, "utf8");
const packageStart = manifest.indexOf("[package]");
const packageEnd = manifest.indexOf("\n[", packageStart + 1);
if (packageStart < 0) {
  throw new Error("Cargo.toml has no [package] section");
}

const end = packageEnd < 0 ? manifest.length : packageEnd;
const packageSection = manifest.slice(packageStart, end);
const currentMatch = packageSection.match(/^version\s*=\s*"([^"]+)"$/m);
if (!currentMatch || !versionPattern.test(currentMatch[1])) {
  throw new Error("Cargo.toml package version must use x.y.z");
}

const current = currentMatch[1];
const next =
  requested ||
  current
    .split(".")
    .map(Number)
    .map((part, index) => (index === 2 ? part + 1 : part))
    .join(".");

if (!versionPattern.test(next)) {
  throw new Error(`Version must use x.y.z, received "${next}"`);
}

const nextPackageSection = packageSection.replace(
  /^version\s*=\s*"[^"]+"$/m,
  `version = "${next}"`,
);
manifest = manifest.slice(0, packageStart) + nextPackageSection + manifest.slice(end);
writeFileSync(manifestPath, manifest);

let lock = readFileSync(lockPath, "utf8");
const lockEntry = /(\[\[package\]\]\nname = "basalt-launcher"\nversion = ")[^"]+(")/;
if (!lockEntry.test(lock)) {
  throw new Error("Cargo.lock has no basalt-launcher package entry");
}
lock = lock.replace(lockEntry, `$1${next}$2`);
writeFileSync(lockPath, lock);

if (process.env.GITHUB_OUTPUT) {
  appendFileSync(process.env.GITHUB_OUTPUT, `version=${next}\n`);
}

console.log(`${current} -> ${next}`);
