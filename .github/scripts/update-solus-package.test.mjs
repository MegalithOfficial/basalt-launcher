import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { renderPackage, writePackage } from "./update-solus-package.mjs";

const SHA256 = "0123456789abcdef".repeat(4);

test("renders a stable Solus recipe", async () => {
  const output = join(await mkdtemp(join(tmpdir(), "basalt-solus-")), "package.yml");
  writePackage("stable", "1.2.3", SHA256, output);

  const recipe = readFileSync(output, "utf8");
  assert.equal(recipe, renderPackage("stable", "1.2.3", SHA256));
  assert.match(recipe, /^name       : basalt-launcher$/m);
  assert.match(recipe, /^version    : 1\.2\.3$/m);
  assert.match(recipe, /releases\/download\/v1\.2\.3\/Basalt\.Launcher_1\.2\.3_linux_x86_64\.tar\.gz/);
  assert.match(recipe, new RegExp(`: ${SHA256}$`, "m"));
  assert.match(recipe, /cp -a usr "\$installdir\/"/);
});

test("renders a separate development package", () => {
  const recipe = renderPackage("dev", "1.2.3", SHA256, "42.2");
  assert.match(recipe, /^name       : basalt-launcher-dev$/m);
  assert.match(recipe, /^version    : 1\.2\.3\.dev\.42\.2$/m);
  assert.match(recipe, /releases\/download\/v1\.2\.3-dev\.42\.2\//);
  assert.match(recipe, /^    - basalt-launcher$/m);
});

test("rejects unsafe release metadata", () => {
  assert.throws(() => renderPackage("nightly", "1.2.3", SHA256), /stable or dev/);
  assert.throws(() => renderPackage("stable", "1.2.3-rc.1", SHA256), /x\.y\.z/);
  assert.throws(() => renderPackage("stable", "1.2.3", "SKIP"), /SHA-256/);
  assert.throws(() => renderPackage("dev", "1.2.3", SHA256, "bad-build"), /run\.attempt/);
  assert.throws(() => renderPackage("stable", "1.2.3", SHA256, "42.1"), /cannot have/);
});
