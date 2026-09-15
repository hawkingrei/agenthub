# Shared MCP Proxy Implementation

## Summary

Slice 9 now has the persistent operation journal, shared JSONL/HTTP/SSE transport, trusted call
preparation, actual HTTP/journal orchestration, and daemon startup recovery. The authenticated MCP
RPC/local stdio proxy, binding resolver, credential isolation, and runtime launch remain incomplete.
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
- A Cargo/Bazel transport crate with bounded framing, explicit protocol versions, legacy lifecycle
  state, HTTP request preparation, streamed responses, and modern parameter header mapping.
- Trusted discovery snapshots, scope-before-digest preparation, declared stable identity, and a
  journaled client that persists raw-response disposition before returning it to the provider.
- Startup recovery through the actual daemon generation, plus an authenticated daemon task fixture
  that holds the execution guard after the requesting caller disconnects.

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
- Store no request/result bodies or arbitrary error strings. Policy derives canonical hashes from
  actual bound arguments and request parameters, preserving the real response separately.
- Build an owned HTTP request before issuing a send permit. Disable reqwest retries/redirects and
  ambient proxy configuration. Keep session/cursor controls outside provider JSONL messages.
- Preserve both legacy lifecycle and modern per-request protocol mechanics. Retain modern MRTR
  payloads unchanged; journal semantics for their continuation rounds remain integration work.
- Preserve a deferred input/task receipt without claiming terminal tool success. Its typed receipt
  cannot be downgraded by transport loss or used to replay the original request. Linked follow-up
  admission is still required before the provider-facing proxy can expose that path.

Stable contract: [MCP operation journal](../features/mcp-operation-journal.md).
Transport contract: [MCP proxy transport](../features/mcp-proxy-transport.md).

## Validation

The following focused commands cover this checkpoint:

```bash
cargo test -p agenthub-db --locked --offline
cargo test -p agenthub-db --locked --offline mcp_operations
cargo test -p agenthub-acp-core -p agenthub-agent-domain --locked --offline
cargo test -p agenthub-mcp --locked --offline
cargo test -p agenthub --lib mcp_send_remains_daemon_owned_after_authenticated_request_disconnects --locked --offline
cargo clippy -p agenthub-db -p agenthub-acp-core -p agenthub-agent-domain --all-targets --locked --offline -- -D warnings
cargo clippy -p agenthub-mcp --all-targets --locked --offline -- -D warnings
cargo fmt --all --check
```

The initial full database suite passed 104 tests. After adding abrupt child-process exit coverage
and scoped event paging, the final journal selection passed 14 tests. ACP core passed 8 tests and
agent domain passed 7 tests. All-target Clippy with warnings denied, formatting, and whitespace
checks passed. The child fixture exits after prepared, sent, and completed commits without running
Rust destructors or closing SQLite. These fixtures do not yet prove a complete MCP transport or an
external service call.

The transport suite passes 22 tests with real local fake servers, including raw TCP truncation
after receipt of a write, redirect non-following, JSON/SSE/JSONL framing, versioned batches,
initialization state, and modern MRTR/header preservation. All-target transport Clippy passes.
The initial sandboxed test attempt could not bind loopback sockets; rerunning with socket permission
passed. This was the raw-transport checkpoint at `ee4ecaa4`.

The subsequent integration fixtures exercise the actual HTTP send and journal together: scoped
arguments, redacted durable rows, unchanged error results, same-identity write retries, rejection
of changed retry parameters, concurrent send exclusion, lost responses across reopen and a fresh
activation, and deferred input/task receipts. The root fixture uses signed execution authentication
and the real daemon task group and operation guard; it is not yet a real MCP RPC or CLI-shim test.
Provider environment isolation and the complete session bridge remain unproven.

Final integration validation passes 32 MCP tests, 15 database journal tests, 8 ACP-core tests,
7 agent-domain tests, the authenticated daemon HTTP ownership test, and 3 daemon-generation tests.
Root/MCP/database/domain all-target Clippy passes with warnings denied; formatting, whitespace,
and local document links pass. The final MCP suite also retains an HTTP JSON-RPC error that omits
its request ID, preserving the original error rather than reporting a transport disconnect.
No local Bazel run or complete provider-facing proxy validation is claimed at this checkpoint.

## Follow-Ups

- Complete slice 9's local stdio shim, authenticated MCP session RPCs, binding resolver and live
  revocation checks, discovery pagination/refresh, linked continuations, credential/environment
  isolation, and provider launch wiring.
- Prove the complete proxy's crash/lost-ACK recovery and legacy static MCP configuration path.
- Integrate existing Mem scope/context bootstrap in slice 10 and app bindings in slice 14 through
  this same journal. Slice 9 remains open in [TODO](../todo.md).
