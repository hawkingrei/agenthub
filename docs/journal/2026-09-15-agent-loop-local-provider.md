# Recoverable Local Loop Providers

## Summary

Slice 5 connects local ACP execution to durable activation ownership, with deterministic local
provider/control/CLI coverage. Product configuration and later rollout slices remain separate.

## Background

Fresh provider sessions must recover canonical Team state through stable actor authority. A process
group alone also cannot establish cleanup when a descendant starts a new session with `setsid`.

## Scope

- Immutable safe launch references and one active Team mailbox partition independent of task attempts.
- Linux executor guardian with a private cleanup receipt and descendant adoption/reaping.
- Fresh and capability-gated resumed ACP startup, required configuration negotiation, and one entry.
- Renewable scoped actor credentials, signed control RPCs, structured finish CLI, and callback cleanup.
- Exclusion of legacy resident prompt injection and direct static MCP from the loop launch path.

## Key Decisions

- Missing cleanup evidence retains ownership, including guardian death or an unobserved daemon crash.
- The guardian is single-threaded and runs before application/runtime startup; provider stdio is
  inherited without adding framing to ACP. The cleanup descriptor is closed across provider exec.
- Actor control operations are daemon-owned through caller disconnects. Per-actor guards remain
  held until admitted requests settle, then cleanup may release the generation.
- Snapshot projection stores a digest and references, never launch arguments, credential envelopes,
  or skill bodies. Different configuration cannot overwrite an activation's resolved launch.
- Credentials use a stable private file with atomic renewal and a lease-based token lifetime. The
  local actor CLI never falls back to legacy authority when a loop envelope is required.
- The entry change is a runtime tail, recovery pointer, and output contract. It adds a bounded
  entry prompt and changes the runtime context label to mailbox identity. Role skill entrypoints
  remain gated for the separate role migration. No role skill is added or renamed in this slice.

## Validation

Focused validation:

- The isolated Linux subreaper experiment adopted, killed, and reaped a `setsid` descendant.
- `cargo test -p agenthub --lib guardian_`: 7 passed with normal local IPC permissions. The Codex
  restricted sandbox rejected Unix socket sends with EPERM; the approved local test run passed.
- `cargo test -p agenthub-db -p agenthub-agent-domain loop_`: 36 DB and 3 domain cases passed.
- `cargo test -p agenthub --lib loop_`: 28 passed, including fresh mailbox recovery, real CLI
  context/finish calls, disconnect ownership, and an unbound fence preserving a legacy writer.
- `cargo test -p agenthub --lib internal::service::`: 52 passed; the thin RPC ownership wrapper
  preserves the existing control operations and their authorization behavior.
- `cargo test -p agenthub --lib agent::manager::`: 99 passed.
- `cargo test -p agenthub-acp loop_`: 6 passed, covering fresh delivery with multiple provider
  updates, supported/unsupported resume, failed resume, rejected profile, and permission cleanup.
- `cargo test -p agenthub-acp permission_`: 13 passed, including late response rejection and
  isolation between exited and live session callbacks. Test selections overlap.
- Root/ACP/store/domain Clippy with warnings denied passed. Documentation file links and
  the generated internal protocol comparison passed. No protocol message changed in this slice.

The full fixture exposed the old CLI inbox fallback to shared-thread runs. Loop inbox recovery now
resolves the signed stable mailbox, while legacy inbox behavior remains unchanged. Binary lookup
tests use explicit inputs instead of mutating process-wide environment during concurrent fixtures.
The Bazel root test declares its actual `agenthub` runtime dependency; local Bazel was not run.
These fixtures do not claim a live paid-provider smoke or hostile same-user process containment.

## Follow-Ups

- Add offline configuration/preflight and membership mutation guards before product enablement.
- Connect canonical work-event intake, standing triggers, the journaled shared proxy, and scoped
  external knowledge. Keep role migration after the required tools and recovery paths exist.
- Add authorized history/workbench controls and the separately gated app/Rara integrations.
