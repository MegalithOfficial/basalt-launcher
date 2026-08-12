# Server hosting

The whole plan: what was decided, what is built, what is left, and the things
that were measured rather than assumed.

A server is not an instance and does not behave like one. It needs no account,
tracks no playtime, publishes no Discord presence, but it accepts commands and
has to be shut down properly. So it is a separate entity with its own list view
and its own detail page. It reuses the existing plumbing (downloads, tasks,
`FileManager`, java discovery, version metadata, UI parts) and none of the
instance code paths.

---

## Decisions

- Servers are their own top level list and their own page, never an instance tab.
- An imported folder is managed in place and never moved.
- When Basalt closes with servers running it asks first, then stops them
  gracefully.
- Connecting to a server from Basalt is out of scope.
- All validation lives on the Rust side. The frontend highlights and shows the
  error it is handed; it parses nothing.
- The launcher owns the process it starts, so a wrapper script is never a direct
  child of the launcher. See Supervisor below.

---

## Data model

`DataRoot::Servers` is a real data root, so Settings shows a movable Servers
entry for free. Managed servers live under `<servers>/<uuid>/`. Imported ones
stay where they are and their absolute path is stored on the row.

**Imported folders are extra capability roots.** `FileManager` touches nothing
outside its cap-std roots, so `Paths` carries extras and `capability_roots()`
includes them, sorted by reverse path length so the longest prefix wins.
`unavailable()` deliberately excludes extras: that check turns into a fatal
dialog at startup, and a server on an unplugged drive must never stop Basalt
from opening. Extra roots are opened best effort, and one that cannot be opened
is logged and skipped rather than silently created.

Two tables, `servers` and `active_server_runs`, plus `server_content_files` and
`server_content_updates` mirroring the instance content tables. Modpack identity
uses the same `pack_provider`, `pack_project_id` and `pack_version_id` fields as
instances. Unlinked imports use `import_source` and `import_source_id`, also like
instances, so a manual server, an imported folder, an imported zip and a pack
installed from Discover do not collapse into the same state. Computed rather
than stored: `dir`, `available`, running state. Cached rather than authoritative:
`cached_port`, `cached_motd`, `cached_max_players`; the config file is the only
truth and these exist so the list can be drawn with one query.

---

## Architecture

### Software registry (`servers/software/`)

Each server software lives in its own file behind one trait: `spec()` (id, label,
hint, java or native, whether it has builds, its config file, its content
directory), `versions()`, `install()`, `detect()` for import, `launch_args()`,
`port()`. Adding a software is a new file and a line in `ALL`. There is no enum
and no match arm to update, and the frontend reads the same registry through
`list_server_software`, so a new software needs no UI change.

`Software` is a handle that can only be built through `find(id)`, so holding one
means the registry knows it. It serializes as its id, which keeps the database
column and the wire format unchanged.

### Provisioning

| Software | What is fetched | Installer? |
|---|---|---|
| Vanilla | `downloads.server` from the version JSON | no |
| Paper | build list from `fill.papermc.io/v3`, then the `server:default` jar | no |
| Purpur | `api.purpurmc.org/v2`, md5 only so verified by size | no |
| Fabric | the prebuilt server launcher from `meta.fabricmc.net` | no |
| NeoForge | `neoforge-<v>-installer.jar` | yes, `--installServer` |
| Forge | `forge-<mc>-<v>-installer.jar` | yes, `--installServer` |
| Pumpkin | the nightly binary for this platform | no |

NeoForge and Forge share `run_installer`, which reports what the installer
produced rather than deciding; only Forge knows about the legacy
`forge-<mc>-<v>.jar` that versions up to 1.16.5 leave behind instead of argument
files. `run.sh` and `run.bat` are never used for launching: the spawned pid would
be the shell, which breaks pid identity, per process metering, and killing on
Windows.

All server downloads go through `provision::fetch`, which is the same shape the
instance installer uses: `set_total`, `download_many_cancellable`, progress into
the task, the task's cancellation token, and retry notices.

### Supervisor process (`servers/control/`)

A server runs under a second process: Basalt's own binary in `--supervise` mode,
which branches before Tauri starts. It owns the child's pipes and listens on
loopback with a token; the launcher connects, receives log lines and sends
commands. It outlives the launcher, so closing or crashing Basalt no longer costs
the console, and on restart the launcher reads a control file for the port and
token and reattaches with commands, live output and a graceful stop intact.

Spawned with `process_group(0)` on unix and `DETACHED_PROCESS |
CREATE_NEW_PROCESS_GROUP` on windows, or it would take the launcher's signals.
The control file holds the token and is owner only.

If the supervisor cannot be spawned the server starts the old way instead, but
only when nothing can have started yet: no control file and the supervisor
process gone. If the control file exists the child is running, and starting a
second server on the same world would be far worse than a lost console.

When the server runs through a pack's start script, the supervisor reports the
Java process under the shell rather than the shell itself, so identity, metering
and killing follow the JVM. It also closes the shell once Java exits, because
these scripts loop and would otherwise bring the server back.

### Runtime

stdin, stdout and stderr are all piped. This is a deliberate break from the
instance approach of writing to a file and tailing it: the console wants low
latency and stdin, and the server already rotates its own `logs/latest.log`, so a
second copy is only disk. Console output is batched at 60ms with a flood cap, and
a rate brake collapses runaway output into one "dropped N lines" marker.

Stopping writes `stop` to stdin, waits, and escalates to a kill. On Windows there
is no SIGTERM, so the stdin pipe is not optional. A server recovered without a
supervisor is read only and says so, with restart and force stop offered.

### Target abstraction (`search/resolve::Target`)

Content installs, dependency resolution, hash identification, update tracking and
removal planning are shared between instances and servers. The target answers
five questions: does it accept this kind, which directory, which rows are
installed, where to record, and what to call it in the task. Everything else,
including the CurseForge browser download flow, is the same code.

---

## Content

**A plugin is not a separate type.** On Modrinth a Paper plugin is
`project_type: "mod"`; the distinguishing thing is the loader token in
`categories`. Measured facts that set the facet rules:

| query | results |
|---|---|
| `categories:paper` | 15978 |
| `categories:paper` + `server_side:required` | 15958 |
| `categories:paper` + `server_side:required OR optional` | 15972 |

So a plugin search applies no environment filter at all, since a Paper plugin is
server side by definition and the strict filter only drops projects that never
filled the field in. A mod search applies `server_side:required OR optional`, so
that mods which are merely optional server side are kept while client only mods
like Sodium are dropped.

On CurseForge plugins are a different class (`classId 5`, Bukkit Plugins) and
mods are class 6, so plugin searches switch the class.

Content goes to `plugins/` for Paper and Purpur, `mods/` for Fabric, NeoForge and
Forge, and nowhere for Vanilla and Pumpkin, which is why the tab is named after
the software and absent when it takes neither.

---

## Modpacks

**Modrinth `.mrpack`** carries `env.server` per file, so a server install drops
only what is marked `unsupported`, unpacks `overrides/` and `server-overrides/`,
and runs the loader's server install using the versions in `dependencies`. The
pack's mods are hash matched and recorded with `origin='pack'`, so they show up
with names, icons and update tracking like an instance's. Which pack and version
a server came from is recorded, so a newer version of it shows on the server
page.

**CurseForge server packs** are a separate optional file and often absent. See
the measured facts below for how they are found; they are not `.mrpack` archives
and do not arrive ready to run.

---

## Shipped

| commit | what |
|---|---|
| `4d90ff1` | the software list loads at startup |
| `4b643d7` | manage the mods or plugins a server already has |
| `dadf2f4` | one card draws installed file rows everywhere |
| `ae11ca5` | install mods and plugins from Modrinth and CurseForge |
| `b45808d` | supervisor process, and player management |
| `b2532d8` | build a server from a modpack, from either side of it |
| `e5eeb5f` | run the start script a pack brings |
| `8568519` | keep modpack server identity, imports and script lifecycle linked |

Also shipped earlier: the servers list and detail page, console with command
input and three layouts, file manager with a validating config editor, the
properties editor (including `pumpkin.toml` as dotted keys), CPU and memory
meters, PumpkinMC, and ANSI colours in the console.

Player management edits `ops.json`, `whitelist.json` and `banned-players.json`
through console commands while the server runs and straight to the files while it
is stopped, with names resolved to uuids, plus a switch for whether the whitelist
is enforced at all.

## Current behavior

A server installed through Get server is linked to its parent modpack version:
`install_server_zip` records `pack_provider`, `pack_project_id` and
`pack_version_id`, the same three fields instances use. Folder and raw zip
imports record their provenance separately. The returned Modrinth server is
linked immediately too, without needing a refresh before its update check works.
CurseForge server packs also reuse the instance manifest planner: files present
in the server ZIP inherit the parent pack's CurseForge project/file records and
`origin = pack`; fingerprint discovery is only a fallback and prefers the
server's linked provider.

Minecraft and loader selectors are locked for every linked modpack server, and
the Rust command rejects stale frontend attempts to change them. Start scripts
are only looked for on linked CurseForge packs. Basalt treats one as an opaque
bootstrap: installer JVMs are ignored, there is no fixed install timeout, and
future launches switch to Basalt's Java command only after a real server JVM
appears and a rescan proves the pack produced complete launch files. Scripts
that detach or produce an unknown shape remain script-managed.

If an existing `user_jvm_args.txt` does not declare both `-Xms` and `-Xmx`,
Rust emits one consent prompt for that server and remembers that it was shown.
Only accepting that prompt, or using the persistent Settings button, writes the
server's saved memory limits into the file. That opt-in is remembered too, so
later memory saves keep the file in step. An open, unedited Files tab reloads
after the Rust writer changes it; unsaved editor text is never overwritten.

That link is what fixes the misdetection: the NeoForge installer leaves its own
`run.sh` in the server folder, so every hand installed Forge or NeoForge server
was being treated as script driven and had its version fields locked.

Force stop now kills the script root before its already collected descendants,
so a restart loop cannot replace Java while it is being stopped. Starting also
refuses to discard a live supervisor control file, and startup failures kill the
whole process tree rather than orphaning the server. The Files tab can reload
both its current directory and an open file.

## Left to do

1. **Broaden the conservative CurseForge handoff.** The platform split is now enforced:
   - Modrinth `.mrpack` states the loader version in `dependencies`. Basalt
     installs it and runs its own command. This already works.
   - A CurseForge server zip does not carry it. Its script bootstraps the first
     launch under the supervisor; after verified launch artifacts appear, Basalt
     owns later Java commands. Unknown script/process shapes deliberately stay
     on the compatibility path. More real packs and detached launchers need
     fixtures before expanding the server-process classifier.

2. **No end to end test for the supervisor.** The wire framing, argument parsing
   and control file are covered. Starting a real child, disconnecting,
   reconnecting and sending a command is not, and that round trip is the feature.

3. **A known flaky test**, `download::tests::cancelling_an_active_download_
   preserves_its_partial_file`, fails roughly one run in five. It binds a real
   socket and cancels on the first progress tick, before the partial file is
   necessarily on disk. Pre existing, not caused by the server work, but it will
   trip `check:rust`.

5. From the original plan: Quilt (the server side is the least documented, so no
   promises before the installer CLI is verified), deriving a server from an
   instance (needs a preview of which client only mods get dropped, read from
   `server_side`, or the feature produces broken servers), backups (the
   `snapshots` module is ready; the work is generalising `instance_id` to an
   owner pair, and a consistent backup of a running server needs `save-off`,
   `save-all`, copy, `save-on`), restarting after a crash (cheap once the runtime
   settled, but it needs a backoff).

6. Phase 3: tunnels. e4mc is a mod, so it only works on modded softwares and
   never on Paper or Vanilla, and its address has to be scraped from the console.
   playit.gg means downloading a third party binary and running it as a second
   managed process, which needs checksum pinning, an explicit consent screen
   naming it, and the ability to see and kill the tunnel process.

---

## Measured, not assumed

- **CurseForge links a server pack with `alternateFileId`.** There is no
  `serverPackFileId` in the response and `isServerPack` is `false` on both files.
  ATM10 Sky, project 1298402: file 7854204 carries `alternateFileId: 7854213`,
  which is `server-2.0.2.zip` at 466 MB and points back with
  `parentProjectFileId: 7854204`.
- **The argument file states both versions.** NeoForge writes
  `--fml.neoForgeVersion 26.2.0.59` and `--fml.mcVersion 26.2` into
  `libraries/net/neoforged/neoforge/<version>/unix_args.txt`. Do not derive the
  Minecraft version from the folder name: the scheme moved from three parts to
  four and Minecraft dropped the leading `1.`.
- **CurseForge server packs do not arrive ready to run.** The ATM10 zip has no
  jar and no `libraries/`, only `mods/`, `config/` and `startserver.sh`. The
  script downloads and installs NeoForge on its first run, and loops with a
  `sleep 10` restart unless told otherwise.
- **ATLauncher sidesteps this rather than solving it.**
  `InstanceInstaller.java:274` extracts the zip, calls `saveServerJson()` and
  returns without installing a loader, and `Server.java:357` launches the pack's
  own `LaunchServer.bat` or `run.bat`. For a CurseForge pack it does not even
  record the mod list or the Java version.
- **Legacy Forge produces no argument files.** The 1.16.5 installer emits
  `forge-1.16.5-36.2.42.jar` instead, found by running it rather than reading
  about it.

---

## Risks

- **Windows has no SIGTERM**, so a graceful stop is stdin or nothing.
- **Java selection**: `java::pick` falls back to the newest runtime when it finds
  no match, and Forge up to 1.16 wants exactly Java 8, so that case is capped and
  warned about rather than started silently.
- **Ports**: binding to test is a courtesy and the race remains, so
  `FAILED TO BIND TO PORT` is surfaced from the console as a first class error.
  Port forwarding and firewalls are out of scope and the UI has to say so, or
  every "my friends cannot connect" becomes a Basalt bug.
- **A missing imported folder** is never fatal: the row shows as missing, actions
  are disabled, and reconnecting reruns the root sync. Removing a server from
  Basalt never touches files; deleting them is a separate, explicitly confirmed
  choice.
- **Windows Job Objects** can kill the supervisor along with the launcher if
  Basalt itself is ever run inside a job with kill on close.

---

## Later: BasaltNode

The long term shape is Pterodactyl like: servers on other machines, with the
launcher as the interface. **The node is authoritative**, so it owns its servers
and the launcher is a client connecting to it.

The `servers` table stays local only and never grows a `node_id`. When nodes
arrive they get their own table and `Server` grows `node: Option<String>`, null
meaning local. Remote servers are never written into the launcher database as
rows; a cache for offline display is legitimate but has to be named a cache and
shown as stale.

The code is already close. The UI is keyed entirely by `server_id`, and
`server:log`, `server:state` and `server:usage` carry nothing local, so a remote
node sending the same payloads changes nothing. `commands/servers.rs` touching
the local world is essentially the node protocol. The brains to move are
`provision`, `properties`, `files` and `import`, and none of them import Tauri or
`AppState`.

The one real obstacle is `runtime.rs`, which emits through `AppHandle` and reads
`AppState`. Emitting has to go behind a narrow interface: Tauri in the launcher,
a socket on the node. **As a rule**, nothing added to those four modules should
take `AppState` or `AppHandle`; take the narrow dependency instead, and
extracting a crate later stays mechanical.
