import { mkdirSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const DESCRIPTION = "A polished Minecraft launcher with practical instance and content management";
const HOMEPAGE = "https://github.com/MegalithOfficial/basalt-launcher";
const DEPENDENCIES = [
  "cairo",
  "desktop-file-utils",
  "gdk-pixbuf2",
  "glib2",
  "gtk3",
  "hicolor-icon-theme",
  "libsoup3",
  "pango",
  "webkit2gtk-4.1",
];

function validate(channel, version, sha256, devBuild) {
  if (channel !== "stable" && channel !== "dev") {
    throw new Error(`Channel must be stable or dev, received "${channel}"`);
  }
  if (!/^\d+\.\d+\.\d+$/.test(version)) {
    throw new Error(`Version must use x.y.z, received "${version}"`);
  }
  if (!/^[a-f0-9]{64}$/.test(sha256)) {
    throw new Error("Debian package SHA-256 must be 64 lowercase hexadecimal characters");
  }
  if (channel === "dev" && !/^\d+\.\d+$/.test(devBuild)) {
    throw new Error(`Development build must use run.attempt, received "${devBuild}"`);
  }
  if (channel === "stable" && devBuild) {
    throw new Error("Stable packages cannot have a development build number");
  }
}

function packageMetadata(channel, version, sha256, devBuild = "") {
  validate(channel, version, sha256, devBuild);
  const development = channel === "dev";
  return {
    packageName: development ? "basalt-launcher-dev-bin" : "basalt-launcher-bin",
    packageVersion: development ? `${version}.dev.${devBuild}` : version,
    releaseTag: development ? `v${version}-dev.${devBuild}` : `v${version}`,
    description: development ? `${DESCRIPTION} (development build)` : DESCRIPTION,
    conflicts: development
      ? ["basalt-launcher", "basalt-launcher-bin"]
      : ["basalt-launcher", "basalt-launcher-dev-bin"],
  };
}

function sourceUrl(version, releaseTag) {
  return `${HOMEPAGE}/releases/download/${releaseTag}/Basalt%20Launcher_${version}_amd64.deb`;
}

export function renderPkgbuild(channel, version, sha256, devBuild = "") {
  const metadata = packageMetadata(channel, version, sha256, devBuild);
  const depends = DEPENDENCIES.map((dependency) => `'${dependency}'`).join(" ");
  const conflicts = metadata.conflicts.map((dependency) => `'${dependency}'`).join(" ");
  return `# Maintainer: MegalithOfficial <gekocakaya@gmail.com>
pkgname=${metadata.packageName}
pkgver=${metadata.packageVersion}
pkgrel=1
pkgdesc="${metadata.description}"
arch=('x86_64')
url="${HOMEPAGE}"
license=('GPL-3.0-only')
depends=(${depends})
makedepends=('libarchive')
provides=('basalt-launcher')
conflicts=(${conflicts})
options=('!strip' '!debug')
source_x86_64=("\${pkgname}-\${pkgver}.deb::${sourceUrl(version, metadata.releaseTag)}")
sha256sums_x86_64=('${sha256}')

package() {
  bsdtar -xf "\${pkgname}-\${pkgver}.deb"
  bsdtar -xf data.tar.* -C "\${pkgdir}"
}
`;
}

export function renderSrcinfo(channel, version, sha256, devBuild = "") {
  const metadata = packageMetadata(channel, version, sha256, devBuild);
  const dependencyLines = DEPENDENCIES.map((dependency) => `\tdepends = ${dependency}`).join("\n");
  const conflictLines = metadata.conflicts.map((dependency) => `\tconflicts = ${dependency}`).join("\n");
  return `pkgbase = ${metadata.packageName}
\tpkgdesc = ${metadata.description}
\tpkgver = ${metadata.packageVersion}
\tpkgrel = 1
\turl = ${HOMEPAGE}
\tarch = x86_64
\tlicense = GPL-3.0-only
\tmakedepends = libarchive
${dependencyLines}
\tprovides = basalt-launcher
${conflictLines}
\toptions = !strip
\toptions = !debug
\tsource_x86_64 = ${metadata.packageName}-${metadata.packageVersion}.deb::${sourceUrl(version, metadata.releaseTag)}
\tsha256sums_x86_64 = ${sha256}

pkgname = ${metadata.packageName}
`;
}

export function writePackage(channel, version, sha256, outputDirectory, devBuild = "") {
  const output = resolve(outputDirectory);
  mkdirSync(output, { recursive: true });
  writeFileSync(resolve(output, "PKGBUILD"), renderPkgbuild(channel, version, sha256, devBuild));
  writeFileSync(resolve(output, ".SRCINFO"), renderSrcinfo(channel, version, sha256, devBuild));
}

const invokedPath = process.argv[1] ? resolve(process.argv[1]) : "";
if (invokedPath === fileURLToPath(import.meta.url)) {
  const [, , channel = "", version = "", sha256 = "", output = "", devBuild = ""] = process.argv;
  if (!output) {
    throw new Error(
      "Usage: update-aur-package.mjs <stable|dev> <version> <deb-sha256> <output-dir> [run.attempt]",
    );
  }
  writePackage(channel, version, sha256, output, devBuild);
}
