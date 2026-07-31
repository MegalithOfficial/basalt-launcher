import { mkdirSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const PACKAGE = "basalt-launcher-bin";
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

function validate(version, sha256) {
  if (!/^\d+\.\d+\.\d+$/.test(version)) {
    throw new Error(`Version must use x.y.z, received "${version}"`);
  }
  if (!/^[a-f0-9]{64}$/.test(sha256)) {
    throw new Error("Debian package SHA-256 must be 64 lowercase hexadecimal characters");
  }
}

function sourceUrl(version) {
  return `${HOMEPAGE}/releases/download/v${version}/Basalt%20Launcher_${version}_amd64.deb`;
}

export function renderPkgbuild(version, sha256) {
  validate(version, sha256);
  const depends = DEPENDENCIES.map((dependency) => `'${dependency}'`).join(" ");
  return `# Maintainer: MegalithOfficial <gekocakaya@gmail.com>
pkgname=${PACKAGE}
pkgver=${version}
pkgrel=1
pkgdesc="${DESCRIPTION}"
arch=('x86_64')
url="${HOMEPAGE}"
license=('GPL-3.0-only')
depends=(${depends})
makedepends=('libarchive')
provides=('basalt-launcher')
conflicts=('basalt-launcher')
options=('!strip' '!debug')
source_x86_64=("\${pkgname}-\${pkgver}.deb::${sourceUrl(version)}")
sha256sums_x86_64=('${sha256}')

package() {
  bsdtar -xf "\${pkgname}-\${pkgver}.deb"
  bsdtar -xf data.tar.* -C "\${pkgdir}"
}
`;
}

export function renderSrcinfo(version, sha256) {
  validate(version, sha256);
  const dependencyLines = DEPENDENCIES.map((dependency) => `\tdepends = ${dependency}`).join("\n");
  return `pkgbase = ${PACKAGE}
\tpkgdesc = ${DESCRIPTION}
\tpkgver = ${version}
\tpkgrel = 1
\turl = ${HOMEPAGE}
\tarch = x86_64
\tlicense = GPL-3.0-only
\tmakedepends = libarchive
${dependencyLines}
\tprovides = basalt-launcher
\tconflicts = basalt-launcher
\toptions = !strip
\toptions = !debug
\tsource_x86_64 = ${PACKAGE}-${version}.deb::${sourceUrl(version)}
\tsha256sums_x86_64 = ${sha256}

pkgname = ${PACKAGE}
`;
}

export function writePackage(version, sha256, outputDirectory) {
  const output = resolve(outputDirectory);
  mkdirSync(output, { recursive: true });
  writeFileSync(resolve(output, "PKGBUILD"), renderPkgbuild(version, sha256));
  writeFileSync(resolve(output, ".SRCINFO"), renderSrcinfo(version, sha256));
}

const invokedPath = process.argv[1] ? resolve(process.argv[1]) : "";
if (invokedPath === fileURLToPath(import.meta.url)) {
  const [, , version = "", sha256 = "", output = ""] = process.argv;
  if (!output) throw new Error("Usage: update-aur-package.mjs <version> <deb-sha256> <output-dir>");
  writePackage(version, sha256, output);
}
