# App Runtime Integration

## Summary

Connect registered HTTP Apps to activation launch and the shared MCP proxy. Freeze manifest versions,
validate native inputs/results, recheck durable authority, and keep credentials daemon-side.

## Background

The registry, management API, and shared result validator were implemented separately. They did not
yet create production mounts or enforce revocation against existing sessions and streams.

## Scope

Local Team launch, offline preflight, trusted HTTP metadata, shared proxy policy, credential
isolation, scoped cleanup, and focused controller/shim/RPC regressions.
Safe discovery Cards and immutable App/version history attribution reuse the same stored selection.

## Key Decisions

- Reuse the journal and transport; the App adapter supplies only trusted manifest and scope policy.
- Check durable grant epochs on open and every admission, plus each listener/subscription delivery
  and idle tick. An admitted operation retains its factual completion.
- Keep effect authority independent of credentials and versions; keep launch fingerprints independent
  of activation IDs while including schema, scope, permission epoch, and workspace changes.
- Strip all registered App credentials from loop child environments, including unused/revoked Apps.
- Increase both launcher and executor session capacity to 32 while retaining global/shared bounds.
- Preserve native argument shape and place trusted identity in separate sensitive HTTP headers.

## Validation

Focused checks pass: 14 root App/API workflows, 45 root MCP regressions, 12 root launch tests,
12 domain tests, 174 DB tests, 14 diagnostics tests, and 9 ACP launch tests. Selections overlap;
child/manual fixtures are reported separately. The real controller/provider/shim/HTTP fixture covers
native invocation, unavailable upstream recovery, and inherited bound/unbound/revoked credentials.

All-target Clippy passes for the root, domain, DB, MCP, ACP, and diagnostics crates with warnings
denied. The final compatibility follow-up passes all three existing Card tests against their independent
synthetic schema after adding the Team membership table it now queries; Clippy passes again.

```bash
cargo +1.96.0 test -p agenthub --lib ::apps::
cargo +1.96.0 test -p agenthub --lib mcp_
cargo +1.96.0 test -p agenthub --lib loop_launch::tests
cargo +1.96.0 test -p agenthub-agent-domain -p agenthub-db -p agenthub-diagnostics --lib
cargo +1.96.0 test -p agenthub-acp loop_launch::tests
cargo +1.96.0 test -p agenthub --lib discovery_card
cargo +1.96.0 clippy -p agenthub -p agenthub-agent-domain -p agenthub-db -p agenthub-mcp -p agenthub-acp -p agenthub-diagnostics --all-targets -- -D warnings
cargo +1.96.0 fmt --all --check
```

The real launch fixture caught a preflight query that assumed Team identity lived directly on the
reservation; it now joins the owning policy. History assertions explicitly distinguish MCP effects
from their enclosing control RPC observations. No production protocol or replay protection was
relaxed to satisfy fixtures.

PR [#1161](https://github.com/hawkingrei/agenthub/pull/1161) passed all applicable checks at
`5fd553b3d7773db94b5401ba68be1b7f83958d29`, including Rust and Bazel coverage, and is ready for review.
Patch coverage is 98.58%; project coverage is 86.57%. No merge was performed.

## Follow-Ups

Signed event ingress remains the following slice. See
[runtime contracts](../features/app-tool-runtime.md) and [active work](../todo.md#agent-loop-product-transition).
