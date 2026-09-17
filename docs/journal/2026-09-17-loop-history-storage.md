# Loop History Storage

## Summary

Add bounded, payload-free activation history projections as the storage foundation for authorized
history APIs and activation diagnostics. This checkpoint does not complete the observability slice.

## Background

Activation, trigger, and lifecycle records already survive executor exit, but current-work queries
require a live execution fence. Historical inspection needs independent pagination and durable scope
checks without recovering raw trigger inputs into the response.

## Scope

- Typed pages for activations, source summaries, and ordered lifecycle events.
- Read transactions bind the Team and actor before reading an activation's sources or events.
- Keyset pagination orders activations by creation time and ID; source and event cursors belong to
  the selected activation. Page sizes are explicitly limited to 1 through 100.
- Source summaries expose typed references, kind, time, and revocation state; original source keys
  and source input objects are omitted.

## Key Decisions

- Keep API authorization above storage scope checks. No public endpoint is exposed at this stage.
- Read history without requiring a running process, active reservation, or current membership.
  A later API must authorize access to the historical Team before invoking these methods.
- Fetch one extra row to distinguish the end of a page without unbounded reads or offset scans.
- Preserve the existing lifecycle event decoder for both execution-context and history reads.
- No schema or persistence-format change is required for these query methods.

## Validation

The focused tests cover concurrent newer activation insertion during pagination, same-time entries,
foreign actor/Team rejection, invalid cursors/limits, database reopen after a recorded outcome and
verified cleanup, complete coalesced-source pagination, event ordering, and source-key exclusion.

```bash
cargo +1.96.0 test -p agenthub-db loop_history --locked --offline
cargo +1.96.0 clippy -p agenthub-db -p agenthub-agent-domain --all-targets --locked --offline -- -D warnings
cargo +1.96.0 test -p agenthub-db -p agenthub-agent-domain --lib --locked --offline
cargo fmt --all --check
git diff --check
```

The two focused history tests pass. Crate all-target Clippy passes with warnings denied, as do all
157 database tests and eight agent-domain tests. The full database selection covers the existing
lifecycle/event readers that now share the history decoder.

## Follow-Ups

- Add Team-authorized history endpoints and API tests for capabilities, foreign scopes, and cursor
  validation; preserve the separate debug-only diagnostic boundary.
- Add safe tool summaries, metrics, diagnostic classification, activation-aware doctor selection,
  and runtime tracing correlation. Existing records do not yet provide a duplicate-suppression
  counter or every actor-control tool duration; record missing facts before exposing those metrics.
- Complete slice 12 validation and publication before treating observability as delivered.
- Contracts: [loop runtime](../features/agent-loop-runtime.md) and
  [runtime diagnostics](../features/runtime-diagnostics.md).
