import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { basename, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

function walk(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    return entry.isDirectory() ? walk(path) : [path];
  });
}

function updaterArtifacts(platform, files) {
  const signed = (file) => existsSync(`${file}.sig`);
  if (platform.startsWith("linux-")) {
    const artifact = files.find((file) => file.endsWith(".AppImage") && signed(file));
    return artifact ? [[platform, artifact]] : [];
  }
  if (platform.startsWith("darwin-")) {
    const artifact = files.find((file) => file.endsWith(".app.tar.gz") && signed(file));
    return artifact ? [[platform, artifact]] : [];
  }
  if (platform.startsWith("windows-")) {
    const nsis =
      files.find((file) => file.endsWith("-setup.exe") && signed(file)) ??
      files.find((file) => file.endsWith(".exe") && signed(file));
    const msi = files.find((file) => file.endsWith(".msi") && signed(file));
    if (!nsis || !msi) return [];
    return [
      [`${platform}-nsis`, nsis],
      [`${platform}-msi`, msi],
      [platform, nsis],
    ];
  }
  return [];
}

export function createManifest(assetsDirectory, repository, tag, version) {
  const root = resolve(assetsDirectory);
  const markers = walk(root).filter((file) => basename(file) === "updater-platform.txt");
  if (markers.length === 0) throw new Error("No updater platform markers were found");

  const platforms = {};
  for (const marker of markers) {
    const platform = readFileSync(marker, "utf8").trim();
    const files = walk(dirname(marker));
    const artifacts = updaterArtifacts(platform, files);
    if (artifacts.length === 0) {
      throw new Error(`No complete set of signed updater artifacts was found for ${platform}`);
    }
    for (const [target, artifact] of artifacts) {
      if (platforms[target]) throw new Error(`Duplicate updater platform ${target}`);

      const name = basename(artifact);
      platforms[target] = {
        signature: readFileSync(`${artifact}.sig`, "utf8").trim(),
        url: `https://github.com/${repository}/releases/download/${tag}/${encodeURIComponent(name)}`,
      };
    }
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
