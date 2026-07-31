<div align="center">
  <img src="public/logo.png" width="96" alt="Basalt logo">
  <h1>Basalt</h1>
  <p>A polished Minecraft launcher that puts form and function on equal footing.</p>

  [![Status](https://img.shields.io/badge/status-alpha-orange)](#project-status)
  [![Platform](https://img.shields.io/badge/platform-Linux-lightgrey)](#project-status)
  [![Tauri](https://img.shields.io/badge/Tauri-2-24C8DB?logo=tauri&logoColor=white)](https://tauri.app/)
</div>

![Basalt launcher](docs/hero.png)

Basalt combines an artwork-led interface with the practical tools expected from a
modern Minecraft launcher. It is designed to feel cohesive and visually considered
without giving up control over instances, loaders, content, accounts, or launch
settings.

## Features

- **Instance management.** Keep saves, mods, resource packs, shaders, settings, and
  launch options separate for every instance.
- **Loader support.** Install and switch between Fabric, Quilt, NeoForge, and Forge.
- **Content discovery.** Browse Modrinth and CurseForge with compatibility filtering,
  dependency resolution, changelogs, and update checks.
- **Modpack installation.** Create an instance directly from a Modrinth modpack.
- **Microsoft accounts.** Manage multiple accounts with device-code sign-in and
  silent token refresh.
- **Skins and capes.** Import, preview, save, and apply skins, then manage capes from
  the same interface.
- **Java detection.** Find compatible runtimes from the system and common version
  managers, with global and per-instance overrides.
- **Visible background work.** Follow downloads and installs, cancel active tasks,
  and recover interrupted operations.
- **Useful diagnostics.** Keep launcher logs separate from game output, change log
  levels, and preview final launch arguments.

## Project status

Basalt is alpha software under active development. Linux is the primary development
and testing platform.

Windows and macOS paths exist through Tauri but have not been properly tested.
CurseForge modpacks are not supported yet, and Mojang's low-resolution version
artwork may appear soft on large windows.

## Install

Release builds are available as AppImage and Debian packages. Arch users can
install `basalt-launcher-bin` from the AUR after the first stable release.

Nix and NixOS users can build and run Basalt directly from source:

```bash
nix run github:MegalithOfficial/basalt-launcher
```

To build the latest development source with development-only features enabled:

```bash
nix run github:MegalithOfficial/basalt-launcher#dev
```

## Running from source

To run it locally, install [Rust](https://rustup.rs/), [Bun](https://bun.sh/), and the
[native dependencies required by Tauri](https://v2.tauri.app/start/prerequisites/).
Then, from the repository root:

```bash
bun install
bun run tauri dev
```

## CurseForge

Modrinth works without additional configuration. CurseForge requires an application
API key.

Sign in at [console.curseforge.com](https://console.curseforge.com/), open the
**API KEYS** tab, and copy the generated key. Paste it under
**Settings → Integrations** in Basalt. Never commit an API key or include one in a
bug report.

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md) before opening a pull request. It covers the
project architecture, code expectations, validation commands, and review process.

## Support

Use [GitHub Issues](https://github.com/MegalithOfficial/basalt-launcher/issues) to
report a reproducible bug or propose a feature. Include the launcher logs, operating
system, Minecraft version, and loader when relevant.
