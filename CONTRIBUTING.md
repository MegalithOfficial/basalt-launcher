# Contributing to Basalt

Basalt is a desktop launcher. Changes can touch account credentials, existing
instances, downloaded code, game files, or child processes. A pull request must be
based on the code that is here, not on how a typical Tauri application might work.

Bug fixes and small, self-contained improvements can go straight to a pull request.
Open an issue before starting a large feature, a new subsystem, a change to stored
data, or anything that can migrate or delete user files.

Report vulnerabilities through [SECURITY.md](SECURITY.md), not a public issue. Project
spaces are covered by [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

## Running the project

Install Rust, Bun, and the
[native dependencies required by Tauri](https://v2.tauri.app/start/prerequisites/).
From the repository root:

```bash
bun install
bun run tauri dev
```

Linux is the primary development platform. If a change is intended for Windows or
macOS, say which platform you actually ran it on.

## Rust owns application behavior

**Rust is the source of truth for Basalt's data and behavior.** Put database access,
aggregation, compatibility rules, provider normalization, install planning,
validation, path decisions, filesystem work, networking, credentials, and process
management in `src-tauri`.

**The frontend displays the result and handles interaction.** React may own form
drafts, open or selected state, animation, presentational formatting, and cheap view-only
operations. It should not reimplement a rule that decides what gets stored,
downloaded, deleted, installed, or launched. If a calculation is based on application
data or could be needed by another screen, do it in Rust and return the shaped result
the UI needs. Existing commands return plans, previews, reports, and summaries for
this reason.

Do not move logic into TypeScript merely because the data is already visible there.
Use TypeScript for a local display concern when sending the work through IPC would
make the implementation needlessly complicated without improving correctness.

## Following a feature through the application

Most features cross the same boundary:

1. Domain behavior lives in the relevant module under `src-tauri/src/`.
2. A command in `src-tauri/src/commands/` validates the request and calls that module.
3. The command is registered in `src-tauri/src/lib.rs`.
4. `src/lib/api.ts` exposes the typed call.
5. Shared response types live in `src/lib/types.ts` and are consumed by the UI.

Keep commands thin. Do not put a second implementation in the command or component.
When a command shape changes, update registration, the API wrapper, TypeScript types,
and every caller in the same pull request.

`AppState` contains the shared database, managed filesystem, credential store,
network manager, running processes, tasks, updates, and presence state. Reuse those
objects instead of opening parallel connections or inventing feature-local global
state.

CPU-heavy parsing and blocking filesystem work must not occupy the async runtime.
The commands use `tokio::task::spawn_blocking` for that work.

## Long-running and destructive work

Installs, imports, repairs, updates, snapshots, and scans belong in the task system.
A task provides progress, cancellation, retry information, activity UI, and records
recoverable operations. Check the cancellation token inside long loops, finish the
task on every result path, and clean up files written before cancellation.

Before changing an instance, use the existing busy checks. A running game or another
active task must not race an import, restore, upgrade, deletion, or repair.

Operations that replace user data need a recoverable sequence. The snapshot and
modpack upgrade code demonstrate the expected pattern: validate first, write into a
staging location, record enough state to recover, switch paths atomically, and keep a
backup until the database and filesystem agree. **Startup recovery is part of the
feature, not optional follow-up work.**

Use transactions when several database writes form one operation. Database migrations
must preserve existing rows, tolerate a partially migrated database, and be safe to
run again. Add `serde` defaults when older stored data or IPC payloads can lack a new
field.

## Files, paths, and media

Use `FileManager` and the checked helpers in `Paths` for launcher-managed files.
Validate identifiers before joining them into a path, reject traversal and symbolic
link escapes, and do not silently overwrite user-selected content. External files
chosen by the user remain untrusted input; inspect them in Rust before copying or
extracting anything.

Archive handling must set limits before allocating or extracting. Reject absolute
paths, traversal, ambiguous separators, links or special entries, duplicate targets,
and data that expands beyond declared bounds. **A frontend file-extension filter is a
convenience, not validation.**

**Do not send image or video bytes through Tauri IPC.** In particular, do not encode
binary media as Base64 or add new `data:` URL payloads to command responses. Return a
filesystem path or remote URL plus metadata. In the frontend, use `mediaSrc` and
`logoSrc` from `src/lib/media.ts`; they handle local asset paths and remote URLs
consistently. Extend that shared module when another reusable media shape needs the
same treatment instead of scattering direct `convertFileSrc` calls through
components. Keep each required directory narrowly listed in the asset protocol scope
in `src-tauri/tauri.conf.json`. Existing Base64-returning paths are legacy exceptions,
not patterns to copy; convert them to path-based media when the feature is changed.

Reuse the native picker and drag-and-drop patterns already used by `UploadModal` and
the helpers in `src/lib/packs.ts`. The frontend should pass selected paths through
IPC, not read the files into JavaScript. Rust must recheck file type, size, structure,
and destination before acting on them. The same applies to save dialogs: let the
frontend choose a destination path and let Rust produce the file.

## Network, credentials, and logs

Use the shared `NetworkManager` and download functions. They centralize proxy and TLS
settings, limits, retries, rate handling, resumable partial files, cancellation, and
checksum verification. Do not create a separate HTTP client for one provider or
write response bodies directly to their final destination.

**Microsoft tokens, provider keys, and proxy passwords belong in `CredentialStore`,
not SQLite, settings JSON, frontend state, or logs.** Values sent to settings screens
are masked. Preserve that separation when adding a credential.

Use structured `tracing` fields and instrument commands where the operation benefits
from it. Skip request fields that can contain secrets, large text, or private paths.
The log redactor is a final safeguard, not permission to log sensitive data. Errors
cross the IPC boundary as user-visible strings, so add useful context without leaking
response bodies, tokens, or complete settings objects.

## Frontend work

Use `src/lib/api.ts` for commands and `src/store.ts` for state shared across views.
Keep state local when it only controls one component. Reuse the existing modal,
confirmation, picker, empty-state, and notification components before creating
another version of the same interaction.

The backend should return the authoritative result after a mutation. Update or
refresh frontend state from that result instead of predicting backend behavior. Show
loading, empty, failure, disabled, and success states where the operation has them.
Destructive actions need an explicit confirmation and must remain locked while the
request is running.

Visual changes should match the surrounding screen rather than introduce an isolated
design system. Include screenshots or a short recording in the pull request.

## Tests and checks

Run the full repository check from the root:

```bash
bun run check
```

It builds the frontend, checks Rust formatting, runs the Rust suite, and runs Clippy
for every target. CI runs it on Linux, Windows, and macOS. Run `bun run format` when
you change Rust and inspect the diff afterward for unrelated formatting.

Rust tests live beside the implementation. Match the existing tests:

- use temporary directories and in-memory databases rather than real launcher data;
- test old and partially migrated data when changing persistence;
- test cancellation, interruption, cleanup, and rollback for multi-step operations;
- test unsafe paths, malformed archives, size limits, and checksum failures at input
  boundaries;
- use local test servers for retry, resume, timeout, and response behavior;
- name tests after the behavior they prove.

There is no automated frontend test runner. UI changes need a manual pass in the
running application. Installation and launch changes need the relevant Minecraft,
loader, Java, and operating-system combinations. State what you tested; do not imply
coverage you did not perform.

## Pull requests

Keep the change focused. Do not mix a feature with unrelated cleanup, formatting, or
dependency updates. Review the complete diff for credentials, launcher data, build
output, debug code, and accidental generated files.

**Commit messages must follow the Conventional Commits format.** Use a type such as
`feat`, `fix`, `perf`, `refactor`, `docs`, `test`, `build`, or `chore`, add a scope
when it makes the change clearer, and describe the actual change in the imperative
mood. Pull request titles should follow the same format. For example:

```text
feat(stats): add a playtime stats page
fix(network): surface provider messages for HTTP failures
perf: stop re-reading whole files and cap caches
```

The description should explain the problem, the implementation, and the observed
result. Include the commands and real workflows you tested. Call out platform limits,
data migrations, compatibility behavior, and follow-up work directly.

## AI-assisted contributions

**AI-assisted coding is allowed. Blind vibecoding is not.**

An AI tool may help investigate code, suggest a focused implementation, explain an
API, or review work the contributor is actively directing. It must not replace the
contributor's understanding or judgment. Do not hand an entire issue to a tool,
repeatedly prompt it until the project compiles, and submit the result without doing
the engineering work yourself. Passing CI does not prove that a change fits Basalt,
preserves user data, or handles failure outside the happy path.

The contributor must inspect the relevant code, choose the approach, review every
generated change, test the real workflow, and be able to explain and maintain the
result. **Material use of generated code must be disclosed in the pull request.**
Small autocomplete, spelling, and formatting assistance does not need disclosure.

A pull request may be closed without further review when the author cannot explain
the code, delegates the whole implementation to AI, relies on invented APIs or
assumptions, introduces broad generated churn, adds tests that merely repeat the
implementation, or passes review comments back to a tool without understanding the
response. Repeated vibecoded submissions may be blocked.

## Reporting bugs

Search existing issues, then use the
[bug report form](https://github.com/MegalithOfficial/basalt-launcher/issues/new?template=bug_report.yml).
A useful report gives someone else enough information to reproduce the failure
without guessing.

Include:

- what you did, what happened, and what should have happened instead;
- the shortest sequence that reproduces it consistently;
- the Basalt version from Settings, or the commit when running from source;
- the operating system and installation format;
- the Minecraft version, loader and loader version, and selected Java runtime when
  they affect the problem;
- whether the issue still occurs on the latest available build.

Attach the relevant launcher log or game-output section when the problem involves a
failed operation, crash, download, install, or launch. Do not attach an entire data
directory or an unrelated full log. Logs are available from Basalt's Logs view. On
Linux, the files are stored under:

```text
~/.local/share/com.megalithofficial.basalt-launcher/
```

When the normal log does not show enough, run a source build with additional detail:

```bash
BASALT_LOG=debug bun run tauri dev
```

**The in-app Share Log flow redacts and previews logs before upload, but review the
result yourself.** Logs copied or attached manually are not automatically sanitized.
Remove access tokens, API keys, account identifiers, usernames, private paths,
server addresses, and anything else you do not want posted publicly.

If the problem only occurs with a particular mod, modpack, world, or provider file,
include its exact name and version. Explain why the failure appears to be in Basalt
rather than in the content or Minecraft itself.

**Never report a security vulnerability in a public issue.** Follow
[SECURITY.md](SECURITY.md) and send the details privately.
