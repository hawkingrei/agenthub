# Direct Runtime Loop Activation

## Summary

The final integration slice now has typed source registration and an initial shared
activation path. This is an implementation checkpoint, not completion of slice 18.

## Background

The managed transport already persisted native input, permissions, history and receipts.
Loop startup still required ACP. The pinned native protocol can register session context
and inline skills but cannot resume across processes or persist an approval owner.

## Scope

- Reuse launch configuration, role selection, credentials, admission and supervised cleanup.
- Register the pinned role/recovery entry and outer identity through a prompt source, and
  managed skills through inline skill sources. Send exactly one activation input afterward.
- Persist source ACKs without attributing a turn or advancing the committed event cursor.
- Keep fresh native identity separate from activation, local launch and stable mailbox identity.
- Distinguish a completed turn, a live input wait and cancellation of a waiting turn.

## Key Decisions

- Require source methods before session creation and validate all source bounds before sends.
  A missing method, rejected source or uncertain receipt prevents activation entry.
- Pin `native-loop-v1` in the launch digest and entry version. Shared role prompt bodies and
  their byte ceiling remain unchanged; the native source adds a bounded outer identity and
  nested-agent authority boundary. No role skill entrypoint changed.
- Resume and configured MCP/App tool bindings remain explicitly unsupported on the pinned
  native build. Disable ambient extensions and native memory instead of borrowing their scope.
- Input-discarded after cancel/interrupt is terminal even if the native peer sends no separate
  turn terminal event. Answered or superseded input is not a cancellation outcome.

## Validation

The protocol suite passes 42 tests, including aggregate limits and capability checks. The
focused source-receipt database test passes. Five native activation cases pass: two fresh
activations with distinct native identities and one shared mailbox, missing structured outcome,
waiting-turn cancellation, rejected source cleanup, and missing registration capability.
The complete manager selection passes 147 tests with 7 opt-in cases ignored. Root library/tests
and protocol all-target Clippy pass with warnings denied.

The opt-in process fixture separately passes against the pinned native binary: prompt and
inline skill registration are accepted and semantic shutdown completes without a model call.
This proves source transport compatibility, not yet a real leader/worker activation.

```bash
cargo test --locked --offline -p agenthub-rara
cargo test --locked --offline -p agenthub-rara native_process_transport_round_trip -- --ignored
cargo test --locked --offline -p agenthub --lib agent::manager::
cargo clippy --locked --offline -p agenthub --lib --tests -- -D warnings
```

## Follow-Ups

Full Card/task context, stable task memory prefixes, controlled tool sources, semantic guard outcome mapping,
and real leader/worker execution remain open. The
[canonical contract](../features/rara-direct-integration.md) and [TODO](../todo.md) track
these boundaries. Slice 17 PR #1164 passes all current-head CI checks, including Bazel
coverage, after correcting the member auto-start fixture race.

## Native Trace Checkpoint

Activation detail and doctor now join only the selected local session to the existing bounded
native history. The snapshot carries stream gaps and uncertain receipts after exit without
exposing event bodies or deriving success from ACK cursors. Doctor uses a read-only connection
and tolerates legacy event databases without native tables. Public detail retains the existing
capability, Team access and local-session ownership checks. All seven activation diagnostic
tests and five history API tests pass, including finished/interrupted native history,
unknown receipts, legacy event databases, redaction and authorization. Root library/tests
and diagnostics Clippy pass with warnings denied.

```bash
cargo test --offline --locked -p agenthub-diagnostics loop_trace::
cargo test --offline --locked -p agenthub --lib loop_history_api_
cargo clippy --offline --locked -p agenthub --lib --tests -p agenthub-diagnostics -- -D warnings
```
