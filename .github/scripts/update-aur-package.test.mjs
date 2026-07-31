import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { renderPkgbuild, renderSrcinfo, writePackage } from "./update-aur-package.mjs";

const SHA256 = "0123456789abcdef".repeat(4);

test("renders matching stable AUR metadata from a release", async () => {
  const output = await mkdtemp(join(tmpdir(), "basalt-aur-"));
  writePackage("stable", "1.2.3", SHA256, output);

  const pkgbuild = readFileSync(join(output, "PKGBUILD"), "utf8");
  const srcinfo = readFileSync(join(output, ".SRCINFO"), "utf8");
  assert.equal(pkgbuild, renderPkgbuild("stable", "1.2.3", SHA256));
  assert.equal(srcinfo, renderSrcinfo("stable", "1.2.3", SHA256));
  assert.match(pkgbuild, /^pkgname=basalt-launcher-bin$/m);
  assert.match(pkgbuild, /releases\/download\/v1\.2\.3\/Basalt\.Launcher_1\.2\.3_amd64\.deb/);
  assert.match(srcinfo, new RegExp(`sha256sums_x86_64 = ${SHA256}`));
});

test("renders matching development AUR metadata from a prerelease", async () => {
  const output = await mkdtemp(join(tmpdir(), "basalt-aur-dev-"));
  writePackage("dev", "1.2.3", SHA256, output, "42.2");

  const pkgbuild = readFileSync(join(output, "PKGBUILD"), "utf8");
  const srcinfo = readFileSync(join(output, ".SRCINFO"), "utf8");
  assert.equal(pkgbuild, renderPkgbuild("dev", "1.2.3", SHA256, "42.2"));
  assert.equal(srcinfo, renderSrcinfo("dev", "1.2.3", SHA256, "42.2"));
  assert.match(pkgbuild, /^pkgname=basalt-launcher-dev-bin$/m);
  assert.match(pkgbuild, /^pkgver=1\.2\.3\.dev\.42\.2$/m);
  assert.match(pkgbuild, /releases\/download\/v1\.2\.3-dev\.42\.2\/Basalt\.Launcher_1\.2\.3_amd64\.deb/);
  assert.match(pkgbuild, /applications\/basalt-launcher\.desktop/);
  assert.match(srcinfo, /conflicts = basalt-launcher-bin/);
});

test("rejects unsafe release metadata", () => {
  assert.throws(() => renderPkgbuild("nightly", "1.2.3", SHA256), /stable or dev/);
  assert.throws(() => renderPkgbuild("stable", "1.2.3-rc.1", SHA256), /x\.y\.z/);
  assert.throws(() => renderSrcinfo("stable", "1.2.3", "SKIP"), /SHA-256/);
  assert.throws(() => renderPkgbuild("dev", "1.2.3", SHA256, "bad-build"), /run\.attempt/);
  assert.throws(() => renderPkgbuild("stable", "1.2.3", SHA256, "42.1"), /cannot have/);
});
