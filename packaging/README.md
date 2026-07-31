# Distribution packaging

Linux release builds produce AppImage and Debian packages. RPM is intentionally
disabled in `src-tauri/tauri.linux.conf.json`.

## Arch User Repository

Stable releases publish `basalt-launcher-bin` from the release Debian package.
Development prereleases publish separately as `basalt-launcher-dev-bin`, so
installing development builds never changes the stable package. The two packages
conflict because they install the same application files.

For both channels, the workflow regenerates `PKGBUILD` and `.SRCINFO` from the
release version and Debian package SHA-256 before pushing to the corresponding
AUR repository.

## Nix and NixOS

The flake builds Basalt from the repository source and its committed npm and
Cargo lock files. It does not download a pre-built Basalt binary.

Run without installing:

```sh
nix run github:MegalithOfficial/basalt-launcher
```

Build and run the latest development source with development-only features enabled:

```sh
nix run github:MegalithOfficial/basalt-launcher#dev
```

Install into the current profile:

```sh
nix profile install github:MegalithOfficial/basalt-launcher
```

The corresponding development profile package is
`github:MegalithOfficial/basalt-launcher#basalt-launcher-dev`.
