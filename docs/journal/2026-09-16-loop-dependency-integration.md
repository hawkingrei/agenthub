# Loop Dependency Integration

## Summary

Combine the reviewed durable scheduling slice with the shared MCP proxy and scoped Mem integration
before implementing role prompts. This checkpoint verifies their common runtime, storage, and CLI
boundaries. It does not complete slice 11's role prompt or skill migration.

## Background

The scheduling branch and the proxy/Mem branch evolved independently after durable work events.
Their histories add adjacent control RPCs, CLI routes, module exports, fixtures, and documentation.
The integration preserves both histories and both feature families rather than dropping either
side at those shared entry points.

## Scope

- Integrate scheduling commit `d88ec11a` with Mem implementation `e228cdc4`.
- Preserve scheduling transactions, admission budgets, cancellation, and scope-change guards.
- Preserve MCP journal, authority checks, native discovery, and unknown-write recovery protection.
- Regenerate internal protocol code with both scheduling and MCP control RPC families.
- Combine the entry prompt's recovery and future-work pointers under version `loop-entry-v5`.

## Key Decisions

- Use normal local merges; do not rewrite reviewed branch history or merge GitHub PRs.
- Keep the existing MCP operation permission alongside member activation/scheduling authority.
- Retain each feature's real provider and signed CLI fixtures, including native Mem startup failures,
  context deadlines, selected learning, and bounded offline scheduling cycles.
- Generate protocol bindings with the unchanged repository build script and compare the tracked
  file with the subsequent actual build output.

## Validation

The integrated source passes root all-target Clippy with warnings denied and an actual binary build.
The full root library selection passes 908 tests; four child helpers are invoked by their parents.
All 155 database and eight agent-domain library tests pass. These selections include legacy APIs,
task writes, scheduling, Mem recovery, operation journaling, and process cleanup boundaries.

```bash
cargo +1.96.0 clippy -p agenthub --all-targets --locked --offline -- -D warnings
cargo +1.96.0 build -p agenthub --bin agenthub --locked --offline
cargo +1.96.0 test -p agenthub --lib --locked --offline
cargo +1.96.0 test -p agenthub-db --lib --locked --offline
cargo +1.96.0 test -p agenthub-agent-domain --lib --locked --offline
cargo fmt --all --check
git diff --check
```

Tests use the existing private command-local Rust temporary directory in `/dev/shm`; full root
tests also use the established command-local loopback proxy bypass. No production settings,
test timeouts, or Bazel configuration were changed for validation. Formatting, whitespace, and
byte equality between tracked and generated protocol bindings are checked independently.

## Follow-Ups

- Implement role-specific loop prompts and their runtime/skill entrypoints, including delegation,
  worker evidence, coordinator acceptance, waiting, and one-entry-prompt recovery proof.
- Finish current-head CI for [Mem PR #1155](https://github.com/hawkingrei/agenthub/pull/1155).
- Continue the remaining observability, UI, App, and Rara slices of the full implementation plan.
- Contracts: [activation](../features/agent-loop-activation-contract.md),
  [scheduling](../features/agent-loop-scheduling.md), and
  [Mem](../features/nowledge-mem-mcp-proxy.md).
