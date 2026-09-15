# Shared MCP Proxy Implementation

## Summary

The first slice 9 checkpoint implements the persistent operation journal. Local stdio proxy,
upstream protocol transport, credential isolation, and runtime launch wiring remain incomplete.
Writable tools remain disabled until those paths and their integration tests are present.

## Background

The loop foundation supplies fenced execution identities, but an upstream write can outlive the
activation that issued it. The journal must preserve uncertainty and late factual results without
giving an expired executor authority to send more work.

## Scope

- Shared agent-domain operation types, including compatibility aliases for existing Mem helpers.
- Additive operation, attempt, and ordered event tables with bounded scoped history reads.
- Atomic preparation/send admission, process-local send permits, immutable replay identity,
  old-daemon recovery, and factual completion independent of executor lifetime.

## Key Decisions

- Reuse the existing loop executor verification inside the journal transaction.
- Record `sent` with SQLite FULL synchronization before returning a permit; no network I/O in a
  transaction. Do not reset the pooled connection to weaker durability afterward.
- Keep daemon generation and attempt generation separate. Only old-daemon sends are recovered;
  ordinary database opens and repeated migrations do not declare a live call unknown.
- Preserve every attempt and transition. Late results can resolve uncertainty, but never replace
  another attempt's result or restore execution permission.
- Enforce original stable caller identity for write retries. A new request ID, profile alias,
  binding revision, or actor cannot hide a prior write in the same effective scope.
- Store no request/result bodies or arbitrary error strings. Protocol policy will construct safe
  digests and preserve the actual response separately.

Stable contract: [MCP operation journal](../features/mcp-operation-journal.md).

## Validation

The following focused commands cover this checkpoint:

```bash
cargo test -p agenthub-db --locked --offline
cargo test -p agenthub-db --locked --offline mcp_operations
cargo test -p agenthub-acp-core -p agenthub-agent-domain --locked --offline
cargo clippy -p agenthub-db -p agenthub-acp-core -p agenthub-agent-domain --all-targets --locked --offline -- -D warnings
cargo fmt --all --check
```

The initial full database suite passed 104 tests. After adding abrupt child-process exit coverage
and scoped event paging, the final journal selection passed 14 tests. ACP core passed 8 tests and
agent domain passed 7 tests. All-target Clippy with warnings denied, formatting, and whitespace
checks passed. The child fixture exits after prepared, sent, and completed commits without running
Rust destructors or closing SQLite. These fixtures do not yet prove a complete MCP transport or an
external service call.

## Follow-Ups

- Complete slice 9's local stdio shim, authenticated daemon transport, upstream protocol handling,
  dynamic discovery, immutable wire intent, credential/environment isolation, and startup recovery.
- Prove fake-upstream disconnect/lost-ACK boundaries and the legacy static MCP configuration path.
- Integrate existing Mem scope/context bootstrap in slice 10 and app bindings in slice 14 through
  this same journal. Slice 9 remains open in [TODO](../todo.md).
