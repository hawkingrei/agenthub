# Loop History Storage

## Summary

Add bounded, payload-free activation history projections, authorized release history APIs, durable
controller RPC/MCP observations, scoped metrics, and activation-aware debug diagnostics. Lifecycle
spans correlate these records through the existing tracing/fastrace bridge. The observability slice
still needs its publication and CI gate.

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

- Scoped metric snapshots cover due work, first admission, per-generation run durations,
  retries/startup failures, cleanup reasons, no-progress evidence, business/registered waits, and Mem.
- Durable duplicate counters do not expand the event log; new exit reasons accompany verified cleanup.
- Debug-only doctor accepts an exact activation or resolves one from its actor. It reports safe
  lease/outcome/continuation evidence, current wake conditions, and distinct loop stall layers.
- Live overlays require the inspected activation's session. Unbound activations do not borrow
  events or permission requests from another execution.
- Lifecycle spans supplement RPC correlation with explicit identities and bounded status fields.

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
- Keep explicit session selectors on the legacy diagnostic path; historical activation selectors
  survive current roster changes and never substitute the actor's latest session.
- Query the latest lifecycle event and the oldest open tool independently of paged summaries, so
  a small display limit cannot hide verified cleanup or a later open boundary.
- A continuation must match the finish receipt, actor/Team, scheduling activation, deadline, and
  task reference. Revocation remains a visible fact rather than being misclassified as lost work.
- Treat the database as read-only during inspection. Metrics are transactional, but the combined
  diagnostic bundle is a series of observations rather than a global atomic snapshot.
- All new spans skip arbitrary function arguments. Private source keys, executor owner IDs,
  credentials, tool inputs/results, and prompts are excluded from their fields.

## Validation

The focused tests cover concurrent newer activation insertion during pagination, same-time entries,
foreign actor/Team rejection, invalid cursors/limits, database reopen after a recorded outcome and
verified cleanup, complete coalesced-source pagination, event ordering, and source-key exclusion.

```bash
cargo +1.96.0 test -p agenthub-db loop_history --locked --offline
cargo +1.96.0 test -p agenthub-db loop_metrics --locked --offline
cargo +1.96.0 test -p agenthub-db loop_tool_history --locked --offline
cargo +1.96.0 test -p agenthub-db mcp_tool_history --locked --offline
cargo +1.96.0 test -p agenthub-mcp --lib --locked --offline
cargo +1.96.0 test -p agenthub --lib loop_history_api --locked --offline
cargo +1.96.0 test -p agenthub --lib loop_history_tracing --locked --offline
cargo +1.96.0 test -p agenthub --lib api::diagnostics::tests --locked --offline
cargo +1.96.0 test -p agenthub-diagnostics -p agenthub-doctor-cli --lib --locked --offline
cargo +1.96.0 test -p agenthub-doctor-cli --lib --release --locked --offline
cargo +1.96.0 check -p agenthub --lib --release --locked --offline
cargo +1.96.0 test -p agenthub --lib internal::service::tests::loop_activation --locked --offline
cargo +1.96.0 clippy -p agenthub -p agenthub-db --all-targets --locked --offline -- -D warnings
cargo +1.96.0 test -p agenthub-db --lib --locked --offline
cargo fmt --all --check
git diff --check
```

Final database regression passes all 166 tests. The four focused tool-history tests cover stale
fences, scoped cursors, cleanup before completion, database reopen, additive/idempotent MCP
migration, deferred input, restart recovery, and a late factual result without replay. The task
lookup regression also checks that a receipt is not success and that its duration is not attributed
to a later asynchronous result.

Proxy regressions pass 111 tests. The final controller selection passes 32 tests plus its
parent-invoked crash helper, and all four history API tests pass, including capability/Team
access, revoked membership, actor removal, invalid cursors, redaction, and tool pagination.
The tool projection stores safe operation IDs directly: a controller-only database does not need
an optional MCP table to inspect its history. Existing loopback/process fixtures require execution
permissions beyond the sandbox; their permission failure was resolved before final validation.

Five metric tests additionally cover concurrent duplicates/rollback/conflicting input, bounded
counters, actor/Team isolation, event windows, exact queue/run/progress/wait/Mem aggregates,
recurring wait rearm, wall-clock regression, unfenced interruption, reopen, and unknown migration
coverage. The metrics API rejects invalid windows and retains historical Team authorization.

The debug diagnostic suite covers the original nine session cases and five activation cases:
read-only reopen after cleanup, coalesced-source pagination, outcome/continuation linkage, pending
and suspended policy, open tools beyond page one, unfenced interruption, historical/current session
isolation, overlay matching, and redaction. CLI selector tests reject mixed activation/session/actor
targets. The two debug HTTP tests cover capability checks plus malformed-selector 400 and
unknown-activation 404 responses.

Lifecycle trace validation runs intake, admission, binding, running, finish, and cleanup against
real storage while capturing structured spans. It checks shared activation identity, generation,
scope, and private-input exclusion. The release CLI test requires immediate rejection before I/O.

Final diagnostic/CLI selections pass 14 and nine debug tests respectively; all ten release CLI
tests pass. Root release library checking succeeds, retaining three existing unused-state warnings
in unchanged SSE code. Root/database/diagnostics/doctor all-target debug Clippy passes with warnings
denied. Formatting, whitespace, and 112 local documentation targets pass.

The initial root release build lost its pre-existing temporary directory. A fresh private `/tmp`
directory resolved the environment failure without changing source or project build configuration.

## Follow-Ups
- Complete slice 12 validation and publication before treating observability as delivered.
- Contracts: [loop runtime](../features/agent-loop-runtime.md) and
  [runtime diagnostics](../features/runtime-diagnostics.md).
