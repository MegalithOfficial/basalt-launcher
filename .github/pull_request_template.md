<!--
Use a Conventional Commit title, for example:
feat(ui): add loader filters to discovery
fix(core): preserve native libraries in inherited versions
-->

## Summary

<!-- What changed? Keep this focused on the behavior reviewers need to understand. -->

## Why

<!-- What problem does this solve, and why is this the right approach for Basalt? -->

## Testing

<!-- List the commands and manual workflows you used. Include Minecraft versions, loaders, and platforms when relevant. -->

- [ ] `bun run check`

## Visual changes

<!-- Add before and after screenshots or a short recording. Remove this section when the change has no visible effect. -->

## Checklist

- [ ] I reviewed the complete diff and removed unrelated changes.
- [ ] I tested the affected workflow in the application.
- [ ] I followed the existing Rust, React, state, and IPC patterns.
- [ ] I updated types, command registration, and callers for any IPC contract change.
- [ ] I added or updated tests where the changed behavior can be tested reliably.
- [ ] I updated documentation when setup, behavior, or contributor expectations changed.
- [ ] I did not include credentials, launcher data, build output, or debug code.
- [ ] I understand and can maintain every line in this pull request, including AI-assisted code.
