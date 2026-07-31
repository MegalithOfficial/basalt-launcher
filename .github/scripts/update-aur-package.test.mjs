import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { renderPkgbuild, renderSrcinfo, writePackage } from "./update-aur-package.mjs";

const SHA256 = "0123456789abcdef".repeat(4);

test("renders matching AUR metadata from a release", async () => {
  const output = await mkdtemp(join(tmpdir(), "basalt-aur-"));
  writePackage("1.2.3", SHA256, output);

  const pkgbuild = readFileSync(join(output, "PKGBUILD"), "utf8");
  const srcinfo = readFileSync(join(output, ".SRCINFO"), "utf8");
  assert.equal(pkgbuild, renderPkgbuild("1.2.3", SHA256));
  assert.equal(srcinfo, renderSrcinfo("1.2.3", SHA256));
  assert.match(pkgbuild, /Basalt%20Launcher_1\.2\.3_amd64\.deb/);
  assert.match(srcinfo, new RegExp(`sha256sums_x86_64 = ${SHA256}`));
});

test("rejects unsafe release metadata", () => {
  assert.throws(() => renderPkgbuild("1.2.3-rc.1", SHA256), /x\.y\.z/);
  assert.throws(() => renderSrcinfo("1.2.3", "SKIP"), /SHA-256/);
});
