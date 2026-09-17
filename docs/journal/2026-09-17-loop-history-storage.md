# Loop History Storage

## Summary

Add bounded, payload-free activation history projections, authorized release history APIs, and
durable controller RPC/MCP observations as the foundation for activation diagnostics. This
checkpoint does not complete the observability slice.

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
- Authorized `/tools` pages contain payload-free RPC and MCP boundary metadata with scoped cursors.
- MCP observations share the canonical send/completion transaction and identify the originating
  operation/attempt without returning private intent, request digests, or completion receipts.
- Existing MCP attempts migrate to safe observations once; later initialization retains their
  identities and never invents historical monotonic durations.

## Key Decisions

- Keep API authorization above storage scope checks. Each release history endpoint requires
  `runtime:inspect` and historical Team access, independently of debug-only diagnostics.
- Read history without requiring a running process, active reservation, or current membership.
  Removed actors remain inspectable by authorized users of their historical Team.
- Fetch one extra row to distinguish the end of a page without unbounded reads or offset scans.
- Preserve the existing lifecycle event decoder for both execution-context and history reads.
- Add an independent tool-observation table. A live fence issues a daemon-owned observation;
  completion can settle after cleanup without authorizing any further executor writes.
- Preserve incomplete observations across restart. Record monotonic durations only while their
  local handles exist, and never infer success from disconnection or process absence.
- Controller RPC spans carry activation, actor, mailbox, and generation attributes through the
  existing tracing subscriber. A stream-opening RPC is not a terminal upstream tool result.

## Validation

The focused tests cover concurrent newer activation insertion during pagination, same-time entries,
foreign actor/Team rejection, invalid cursors/limits, database reopen after a recorded outcome and
verified cleanup, complete coalesced-source pagination, event ordering, and source-key exclusion.

```bash
cargo +1.96.0 test -p agenthub-db loop_history --locked --offline
cargo +1.96.0 test -p agenthub-db loop_tool_history --locked --offline
cargo +1.96.0 test -p agenthub-db mcp_tool_history --locked --offline
cargo +1.96.0 test -p agenthub-mcp --lib --locked --offline
cargo +1.96.0 test -p agenthub --lib loop_history_api --locked --offline
cargo +1.96.0 test -p agenthub --lib internal::service::tests::loop_activation --locked --offline
cargo +1.96.0 clippy -p agenthub -p agenthub-db --all-targets --locked --offline -- -D warnings
cargo +1.96.0 test -p agenthub-db --lib --locked --offline
cargo fmt --all --check
git diff --check
```

Final database regression passes all 161 tests. The four focused tool-history tests cover stale
fences, scoped cursors, cleanup before completion, database reopen, additive/idempotent MCP
migration, deferred input, restart recovery, and a late factual result without replay. The task
lookup regression also checks that a receipt is not success and that its duration is not attributed
to a later asynchronous result.

Proxy regressions pass 111 tests. The final controller selection passes 32 tests plus its
parent-invoked crash helper, and all three history API tests pass, including capability/Team
access, revoked membership, actor removal, invalid cursors, redaction, and tool pagination.
The tool projection stores safe operation IDs directly: a controller-only database does not need
an optional MCP table to inspect its history. Existing loopback/process fixtures require execution
permissions beyond the sandbox; their permission failure was resolved before final validation.

Root/database all-target Clippy passes with warnings denied. Formatting, whitespace, and all 104
local documentation targets pass.

## Follow-Ups

- Add metrics, diagnostic classification, activation-aware doctor selection, and the remaining
  lifecycle tracing correlation. Persist missing facts such as
  duplicate-suppression counters before exposing their metrics.
- Complete slice 12 validation and publication before treating observability as delivered.
- Contracts: [loop runtime](../features/agent-loop-runtime.md) and
  [runtime diagnostics](../features/runtime-diagnostics.md).
