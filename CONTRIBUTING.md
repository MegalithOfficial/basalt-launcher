# Contributing to Basalt

Thank you for helping improve Basalt.

Basalt is still in alpha, and the codebase is moving quickly. Contributions are
welcome, whether they fix a small papercut, improve reliability, or add a larger
launcher feature. The best changes are focused, fit the existing architecture, and
are tested in the real application before review.

For large features, architecture changes, or work that affects user data, open an
issue before investing in an implementation. This gives us a chance to agree on the
direction and avoid duplicated work.

## Getting started

Basalt is a Tauri 2 application with a React and TypeScript frontend and a Rust
backend.

You will need:

- [Rust](https://rustup.rs/)
- [Bun](https://bun.sh/)
- The [Tauri system dependencies](https://v2.tauri.app/start/prerequisites/) for your
  platform

Clone your fork, then install the frontend dependencies from the repository root:

```bash
bun install
```

Start the application in development mode:

```bash
bun run tauri dev
```

The frontend uses Vite and supports hot module replacement. Tauri rebuilds and
restarts the Rust application when backend code changes. Compiler output appears in
the terminal running the development command.

Basalt is currently developed and tested primarily on Linux. Changes for Windows or
macOS are welcome, but explain what you were able to test on that platform.

## Project structure

The repository is split into two main layers:

```text
src/
├── components/        Shared React components
├── components/project/
│   └── ...            Project and version browser components
├── lib/               IPC bindings, types, logging, and UI helpers
├── views/             Top-level application views
└── store.ts           Shared Zustand state and application actions

src-tauri/src/
├── auth/              Microsoft and Minecraft authentication
├── download/          Download retries, verification, and cancellation
├── install/           Minecraft installation
├── launch/            Launch arguments and process supervision
├── loaders/           Fabric, Quilt, NeoForge, and Forge support
├── logging/           Structured logs and the in-app log buffer
├── meta/              Minecraft version manifests and metadata
├── modpack/           Modpack installation
├── search/            Modrinth and CurseForge integrations
├── skin/              Skin and cape management
├── commands.rs        Tauri command boundary
├── db.rs              SQLite schema and persistence
├── tasks.rs           Long-running task state and progress
└── lib.rs             Application setup and command registration
```

Before introducing a new pattern, look for an existing feature with similar data
flow. Reusing the established path usually produces a smaller and easier-to-review
change.

## Architecture

Rust owns the parts of Basalt that interact with the system or define application
behavior. This includes authentication, networking, persistence, filesystem access,
Minecraft installation, loader support, process management, validation, and other
business logic.

React owns presentation, user interaction, navigation, and short-lived view state.
Avoid duplicating backend rules in components or using the frontend as a second
source of truth.

Tauri commands are the boundary between these layers. When adding or changing a
command, follow the complete path:

1. Put the implementation in the relevant Rust module.
2. Keep the function in `src-tauri/src/commands.rs` focused on the IPC boundary.
3. Register new commands in `src-tauri/src/lib.rs`.
4. Add or update the typed wrapper in `src/lib/api.ts`.
5. Update the shared TypeScript types and every affected caller.

Preserve compatibility across the boundary intentionally. A renamed field or changed
nullability is a contract change, even if each side still compiles on its own.

Long-running work must not block the UI thread. Use the existing task and event
infrastructure for installs, downloads, updates, and launches so progress,
cancellation, recovery, and errors remain consistent throughout the app.

## Coding guidelines

Match the surrounding code and keep the scope of the change easy to follow.

- Prefer a direct implementation over a new abstraction with only one use.
- Keep unrelated cleanup and refactoring out of feature and bug-fix pull requests.
- Follow the surrounding formatting and do not reformat unrelated files.
- Use clear names and add comments only when they explain intent that the code cannot.
- Do not leave dead code, placeholder behavior, debug output, or local workarounds in
  a submitted change.
- Preserve user data and configuration unless the change explicitly requires a
  migration or deletion path.
- Surface failures with useful context. Do not silently ignore an error that changes
  user-visible behavior.

### Rust

- Keep filesystem, network, database, and launcher behavior in the relevant backend
  module rather than in `commands.rs`.
- Use the shared error types and existing state instead of creating parallel error or
  storage paths.
- Use `tracing` for backend diagnostics, with structured fields where possible:

  ```rust
  tracing::info!(instance_id, version_id, "install finished");
  ```

- Instrument important operations consistently with the surrounding code.
- Never log access tokens, API keys, complete settings objects, or other secrets.
- Verify downloaded artifacts when the provider supplies a hash.

### React and TypeScript

- Keep components focused on rendering and interaction. Put shared IPC calls in
  `src/lib/api.ts` and shared application state in `src/store.ts`.
- Select individual Zustand values where practical. Avoid selectors that construct a
  new object or array on every store update.
- Keep TypeScript strict. Do not bypass a type error with `any` or suppression unless
  the reason is unavoidable and documented.
- The `cn` helper only joins class names. It does not resolve conflicting Tailwind
  utilities, so place conflicting classes in mutually exclusive branches.
- Follow the existing visual language. New UI should feel like part of Basalt, not a
  separate design system.

## Testing your change

Run the checks that apply to your work from the repository root:

```bash
bun run check
```

This runs both `check:frontend` and `check:rust`. The frontend check compiles
TypeScript and creates a production Vite build. The Rust check runs the test suite
and checks every target with Clippy.

Rust tests live beside the code in `#[cfg(test)]` modules. Add focused tests for
parsing, path handling, version rules, migrations, dependency resolution, and other
deterministic behavior. Tests involving files or SQLite should use isolated temporary
paths and must not touch a contributor's real launcher data.

There is currently no automated frontend test runner. For UI changes, run the app and
manually verify the affected workflow, including loading, empty, success, and error
states where relevant. For installation or launch changes, test with the Minecraft
version and loader combinations your patch affects.

A test should prove the behavior under change. If you add a regression test, confirm
that it fails without the fix.

## Using AI tools

AI tools may be used to assist with a contribution, but they are not a substitute for
understanding the codebase or doing the engineering work. Do not submit generated
changes that you have only prompted, skimmed, or tested through trial and error.

You are responsible for every line in your pull request. Read the complete diff,
understand why the implementation works, remove generated clutter, and verify the
result yourself. Do not submit code you cannot explain or maintain.

Treat generated code as untrusted until you have reviewed and validated it. Check for
invented APIs, unnecessary abstractions, missed call sites, tests that do not exercise
the change, and behavior copied from a different architecture.

Pull requests that appear to contain unread or unverified generated code may be
closed. Review time is limited, and contributors are expected to do that review
before asking maintainers to do it for them.

## Commits

Use [Conventional Commits](https://www.conventionalcommits.org/) with a concise,
lowercase summary:

```text
feat(ui): add loader filters to discovery
fix(core): preserve native libraries in inherited versions
docs: clarify the local development workflow
```

Common scopes in this repository include `core`, `ui`, `settings`, and `accounts`.
Use a different scope when it communicates the change more accurately. Add a commit
body when the reason, migration, or tradeoff is not clear from the summary.

Keep commits coherent. Reviewers should be able to understand why each commit exists
without separating it from unrelated formatting or cleanup.

## Opening a pull request

Before opening a pull request:

- Rebase or merge the latest `main` branch and resolve conflicts in your branch.
- Review the full diff for accidental files, debug code, secrets, and unrelated
  changes.
- Run the relevant checks and test the feature in the application.
- Update documentation when behavior, setup, or contributor expectations change.
- Keep generated files, build output, launcher data, and credentials out of the
  repository.

In the pull request description, explain:

- What changed
- Why the change is needed
- How you tested it
- Which platforms, Minecraft versions, and loaders were tested when relevant
- Any known limitation, follow-up work, migration, or tradeoff

Small, focused pull requests are easier to review and merge. If review reveals a
broader cleanup opportunity, prefer a separate pull request unless it is required for
the current change.

Review is a technical conversation. Respond to feedback, ask when a request is
unclear, and push follow-up commits that keep the discussion easy to trace.

## Reporting bugs

Search the existing issues before opening a new report. Include enough information
for someone else to reproduce the problem:

- What you expected and what happened instead
- Exact steps to reproduce it
- Your operating system and Basalt version or commit
- The Minecraft version, loader, and loader version when relevant
- Whether Java was selected automatically or configured manually
- Relevant launcher logs and error messages

Logs are available from the Logs view and from the launcher's data directory. On
Linux, the default data directory is:

```text
~/.local/share/com.megalithofficial.basalt-launcher/
```

Set `BASALT_LOG=debug` before starting Basalt when a normal log does not contain
enough detail:

```bash
BASALT_LOG=debug bun run tauri dev
```

Remove access tokens, API keys, usernames, local paths, and any other private
information before attaching logs.

## Thank you

Good launcher code has to handle unreliable networks, changing upstream services,
many Minecraft versions, and real user data. Careful reports, focused patches, and
thoughtful reviews all make Basalt better.
