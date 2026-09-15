# Offline Loop Configuration

## Summary

Slice 6 separates offline Team configuration from execution, adds explicit loop policy controls and
shared preflight, and protects membership/workspace changes with transactional ownership guards.
Legacy eager startup remains covered by compatibility tests.

## Background

Team creation and roster edits previously started resident processes immediately. Process status
alone also cannot authorize moving a member with retained activation or task ownership.

## Scope

- Explicit `spec.execution_mode = "loop"`, with disabled member policies created atomically.
- Owner-authorized member loop configuration and safe discovery Card projection.
- Shared preflight for enablement and local launches, including required capability failures.
- Per-actor configuration/start exclusion and durable scope mutation checks.
- Copying a Card with a new identity, plus guarded membership/workspace/deletion paths.

## Key Decisions

- Enabling admission does not manufacture a work trigger. Suspension remains available even when
  provider preflight fails and does not stop an already admitted activation.
- Legacy Team configuration retains its eager startup behavior. Loop configuration does not inject
  the resident default prompt or synthetic phase steps; role prompt migration remains slice 11.
- Configuration operations remain daemon-owned across caller disconnects and serialize with starts.
  Store guards run inside the same write transaction as membership or workspace changes.
- Session continuity changes wait for execution cleanup. Budget updates retain the lease duration
  captured by an active reservation. Direct ACP mutations cannot change a live activation snapshot.
- Expired reservations and unreleased task claims still block scope changes. Canonical pending mail,
  reply obligations, task ownership, and permissions must be reconciled before mutation.
- Retained activation history keeps its original scope. Empty detached policies can be removed;
  a historical identity cannot be silently rebound to another Team. Copying creates a new identity.
- Card copies bind the member placeholder to the actual newly created Agent ID. They copy neither
  policy generations nor sessions, mailbox identity, claims, activations, or credentials.
- Required MCP/Mem capabilities fail explicitly until their later proxy integrations are available.
  Resume support is negotiated before the activation entry, and never falls back after a failed load.
- Team optimistic-update timestamps advance monotonically so consecutive edits in one second cannot
  reuse the same compare-and-swap value.

## Validation

- 42 root loop tests passed, including 14 configuration tests and the real CLI/fake ACP recovery
  fixtures. Coverage includes disconnect/start exclusion, concurrent removal/intake, retained
  history, reply obligations after acknowledgment, task/permission guards, preflight, and authority.
- 22 legacy Team API tests and 101 AgentManager tests passed; selections overlap the root loop set.
- 40 control-store loop tests passed, including active-lease budget compatibility and the atomic
  session-policy guard. An initially overbroad budget guard was narrowed to preserve that contract.
- Root/store all-targets Clippy with warnings denied passed; the final store-only adjustment was
  rechecked. Cargo formatting, explicit formatting of the included API tests, patch whitespace,
  and local documentation links passed.
- Tests requiring local provider/guardian IPC used normal local socket permissions because the
  restricted Codex sandbox rejects Unix socket sends. No paid-provider or browser smoke was needed
  for this backend configuration slice. Local Bazel was not run; remote checks validate its targets.
- Full CI exposed an additional file-backed conversation test fixture that did not initialize loop
  tables before creating a Team. Reuse the production loop migration in that fixture so the test
  exercises the current configuration guard schema without weakening production checks.
  The focused concurrent append/drain/read regression passed with the corrected fixture.

## Follow-Ups

- Connect canonical work-event intake and bounded standing/dependency triggers in slices 7-8.
- Add the journaled MCP proxy and scoped Mem before migrating the role entrypoints.
