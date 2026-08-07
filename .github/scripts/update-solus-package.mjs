import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const DESCRIPTION = "A polished Minecraft launcher with practical instance and content management";
const HOMEPAGE = "https://github.com/MegalithOfficial/basalt-launcher";

function validate(channel, version, sha256, devBuild) {
  if (channel !== "stable" && channel !== "dev") {
    throw new Error(`Channel must be stable or dev, received "${channel}"`);
  }
  if (!/^\d+\.\d+\.\d+$/.test(version)) {
    throw new Error(`Version must use x.y.z, received "${version}"`);
  }
  if (!/^[a-f0-9]{64}$/.test(sha256)) {
    throw new Error("Payload SHA-256 must be 64 lowercase hexadecimal characters");
  }
  if (channel === "dev" && !/^\d+\.\d+$/.test(devBuild)) {
    throw new Error(`Development build must use run.attempt, received "${devBuild}"`);
  }
  if (channel === "stable" && devBuild) {
    throw new Error("Stable packages cannot have a development build number");
  }
}

function metadata(channel, version, devBuild = "") {
  const development = channel === "dev";
  return {
    name: development ? "basalt-launcher-dev" : "basalt-launcher",
    version: development ? `${version}.dev.${devBuild}` : version,
    releaseTag: development ? `v${version}-dev.${devBuild}` : `v${version}`,
    summary: development ? "Basalt Launcher development build" : "Basalt Launcher",
    description: development ? `${DESCRIPTION} (development build).` : `${DESCRIPTION}.`,
    conflict: development ? "basalt-launcher" : "basalt-launcher-dev",
  };
}

export function renderPackage(channel, version, sha256, devBuild = "") {
  validate(channel, version, sha256, devBuild);
  const packageMetadata = metadata(channel, version, devBuild);
  const payloadName = `Basalt.Launcher_${version}_linux_x86_64.tar.gz`;
  const source = `${HOMEPAGE}/releases/download/${packageMetadata.releaseTag}/${payloadName}`;

  return `name       : ${packageMetadata.name}
version    : ${packageMetadata.version}
release    : 1
source     :
    - ${source} : ${sha256}
homepage   : ${HOMEPAGE}
license    : GPL-3.0-only
component  : games
summary    : ${packageMetadata.summary}
description: |
    ${packageMetadata.description}
conflicts  :
    - ${packageMetadata.conflict}
install    : |
    cp -a usr "$installdir/"
`;
}

export function writePackage(channel, version, sha256, outputFile, devBuild = "") {
  const output = resolve(outputFile);
  mkdirSync(dirname(output), { recursive: true });
  writeFileSync(output, renderPackage(channel, version, sha256, devBuild));
}

const invokedPath = process.argv[1] ? resolve(process.argv[1]) : "";
if (invokedPath === fileURLToPath(import.meta.url)) {
  const [, , channel = "", version = "", sha256 = "", output = "", devBuild = ""] = process.argv;
  if (!output) {
    throw new Error(
      "Usage: update-solus-package.mjs <stable|dev> <version> <payload-sha256> <output-file> [run.attempt]",
    );
  }
  writePackage(channel, version, sha256, output, devBuild);
}
