# Basalt

Basalt is a Minecraft launcher built around a simple idea: the launcher should look
like the game you are about to play, not like a settings dialog. Your instance's
banner fills the window edge to edge, the interface tints itself from the colors of
that artwork, and the chrome stays out of the way.

## What it does

**Instances.** Every instance is fully isolated: its own saves, resource packs,
shaders, mods, and options, with per-instance memory and Java overrides. Versions and
loaders can be switched after creation, with honest warnings about what a switch
breaks. Playtime and last-played are tracked per instance.

**Loaders.** Fabric, Quilt, NeoForge, and Forge are installed and configured by the
launcher. Fabric and Quilt install from their official metadata in seconds; NeoForge
and Forge run the official installer headlessly against Basalt's data directory.

**Content.** Mods, resource packs, and shaders are searched, browsed, and installed
from Modrinth out of the box, and from CurseForge with your own API key. Project pages
carry the full description, gallery, and a versions browser that knows which build
your instance runs, which builds are compatible, and which one is an update. Required
dependencies are detected and offered before anything extra is downloaded. Everything
installed stays linked to its source, so the launcher always knows what exact version
of what project is sitting in your mods folder.

**Accounts.** Microsoft sign-in through the device code flow, multiple accounts, and
token refresh handled quietly in the background. Your character's actual skin shows up
as your avatar.

**The console.** Launches stream the game's output live with severity coloring,
process state, and playtime capture on exit. Crashes leave the log open for reading.

## How it is built

The backend is Rust inside Tauri 2. It talks directly to the official sources: Mojang's
piston-meta for versions and assets, launchercontent for per-version key art, the
Fabric and Quilt metadata services, the Forge and NeoForge installers, and the Modrinth
and CurseForge APIs. Every downloaded artifact with a published hash is SHA1 verified,
downloads run concurrently with resume-by-skip, and state lives in a SQLite database.
The interface is React with Tailwind, and the accent color that runs through the whole
app is extracted from your banner image in Rust at install time.

## Status

Basalt is in active development and not yet packaged for release. It is developed and
tested on Linux; the full loop works today: sign in, create an instance on any version,
add a loader and mods, play, and watch the console. CurseForge search requires a free
API key from console.curseforge.com, entered in Settings, because their API is keyed
per application. Expect sharp edges.

## Running it

You need Rust, Bun, and webkit2gtk. `bun install` once, then `bun run tauri dev` for a
development build or `bun run tauri build` to produce a bundle.
