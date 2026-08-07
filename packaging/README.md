# Distribution packaging

Linux release builds produce AppImage and Debian packages. RPM is intentionally
disabled in `src-tauri/tauri.linux.conf.json`. Releases also contain a neutral
Linux payload used by distribution recipes.

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

## Solus

Each release includes `basalt-launcher-solus.package.yml` and the Linux payload
referenced by that recipe. On a configured Solus packaging checkout, copy the
recipe into the package directory as `package.yml` and run:

```sh
go-task
```

The resulting `.eopkg` installs the same application files as the other Linux
packages and leaves updates to eopkg. Development releases use the separate
`basalt-launcher-dev` package name so they cannot replace a stable installation
by accident.
