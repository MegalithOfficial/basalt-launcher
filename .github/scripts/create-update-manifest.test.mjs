import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { createManifest } from "./create-update-manifest.mjs";

function platform(root, name, artifact, signature) {
  const directory = join(root, name);
  mkdirSync(directory, { recursive: true });
  writeFileSync(join(directory, "updater-platform.txt"), name);
  writeFileSync(join(directory, artifact), "bundle");
  writeFileSync(join(directory, `${artifact}.sig`), signature);
  return directory;
}

test("creates a signed manifest for each build platform", () => {
  const root = mkdtempSync(join(tmpdir(), "basalt-updater-"));
  platform(root, "linux-x86_64", "Basalt Launcher.AppImage", "linux-signature");
  const windows = platform(
    root,
    "windows-x86_64",
    "Basalt Launcher-setup.exe",
    "windows-nsis-signature",
  );
  writeFileSync(join(windows, "Basalt Launcher.msi"), "bundle");
  writeFileSync(join(windows, "Basalt Launcher.msi.sig"), "windows-msi-signature");
  platform(root, "darwin-aarch64", "Basalt Launcher.app.tar.gz", "mac-signature");

  assert.deepEqual(createManifest(root, "MegalithOfficial/basalt-launcher", "v1.2.3", "1.2.3"), {
    version: "1.2.3",
    notes: "https://github.com/MegalithOfficial/basalt-launcher/releases/tag/v1.2.3",
    platforms: {
      "linux-x86_64": {
        signature: "linux-signature",
        url: "https://github.com/MegalithOfficial/basalt-launcher/releases/download/v1.2.3/Basalt.Launcher.AppImage",
      },
      "windows-x86_64": {
        signature: "windows-nsis-signature",
        url: "https://github.com/MegalithOfficial/basalt-launcher/releases/download/v1.2.3/Basalt.Launcher-setup.exe",
      },
      "windows-x86_64-nsis": {
        signature: "windows-nsis-signature",
        url: "https://github.com/MegalithOfficial/basalt-launcher/releases/download/v1.2.3/Basalt.Launcher-setup.exe",
      },
      "windows-x86_64-msi": {
        signature: "windows-msi-signature",
        url: "https://github.com/MegalithOfficial/basalt-launcher/releases/download/v1.2.3/Basalt.Launcher.msi",
      },
      "darwin-aarch64": {
        signature: "mac-signature",
        url: "https://github.com/MegalithOfficial/basalt-launcher/releases/download/v1.2.3/Basalt.Launcher.app.tar.gz",
      },
    },
  });
});

test("rejects an unsigned updater artifact", () => {
  const root = mkdtempSync(join(tmpdir(), "basalt-updater-"));
  const directory = join(root, "linux-x86_64");
  mkdirSync(directory, { recursive: true });
  writeFileSync(join(directory, "updater-platform.txt"), "linux-x86_64");
  writeFileSync(join(directory, "Basalt.AppImage"), "bundle");

  assert.throws(
    () => createManifest(root, "MegalithOfficial/basalt-launcher", "v1.2.3", "1.2.3"),
    /No complete set of signed updater artifacts/,
  );
});

test("rejects Windows updates unless both installer families are signed", () => {
  const root = mkdtempSync(join(tmpdir(), "basalt-updater-"));
  platform(root, "windows-x86_64", "Basalt Launcher-setup.exe", "windows-signature");

  assert.throws(
    () => createManifest(root, "MegalithOfficial/basalt-launcher", "v1.2.3", "1.2.3"),
    /No complete set of signed updater artifacts was found for windows-x86_64/,
  );
});
