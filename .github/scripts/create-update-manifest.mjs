import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { basename, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

function walk(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    return entry.isDirectory() ? walk(path) : [path];
  });
}

function updaterArtifact(platform, files) {
  const signed = (file) => existsSync(`${file}.sig`);
  if (platform.startsWith("linux-")) {
    return files.find((file) => file.endsWith(".AppImage") && signed(file));
  }
  if (platform.startsWith("darwin-")) {
    return files.find((file) => file.endsWith(".app.tar.gz") && signed(file));
  }
  if (platform.startsWith("windows-")) {
    return (
      files.find((file) => file.endsWith("-setup.exe") && signed(file)) ??
      files.find((file) => file.endsWith(".exe") && signed(file))
    );
  }
  return undefined;
}

export function createManifest(assetsDirectory, repository, tag, version) {
  const root = resolve(assetsDirectory);
  const markers = walk(root).filter((file) => basename(file) === "updater-platform.txt");
  if (markers.length === 0) throw new Error("No updater platform markers were found");

  const platforms = {};
  for (const marker of markers) {
    const platform = readFileSync(marker, "utf8").trim();
    const files = walk(dirname(marker));
    const artifact = updaterArtifact(platform, files);
    if (!artifact) throw new Error(`No signed updater artifact was found for ${platform}`);
    if (platforms[platform]) throw new Error(`Duplicate updater platform ${platform}`);

    const name = basename(artifact);
    platforms[platform] = {
      signature: readFileSync(`${artifact}.sig`, "utf8").trim(),
      url: `https://github.com/${repository}/releases/download/${tag}/${encodeURIComponent(name)}`,
    };
  }

  return {
    version,
    notes: `https://github.com/${repository}/releases/tag/${tag}`,
    platforms,
  };
}

const invokedPath = process.argv[1] ? resolve(process.argv[1]) : "";
if (invokedPath === fileURLToPath(import.meta.url)) {
  const [, , assets = "", repository = "", tag = "", version = "", output = ""] = process.argv;
  if (!assets || !repository || !tag || !version || !output) {
    throw new Error(
      "Usage: create-update-manifest.mjs <assets-dir> <owner/repo> <tag> <version> <output>",
    );
  }
  const destination = resolve(output);
  mkdirSync(dirname(destination), { recursive: true });
  writeFileSync(destination, `${JSON.stringify(createManifest(assets, repository, tag, version), null, 2)}\n`);
}
