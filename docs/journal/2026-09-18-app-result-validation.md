# App Tool Result Validation

## Summary

The shared proxy/journal now accepts an optional trusted completed-result validator. It validates
immediate, batched, continued, and deferred results before accepting their tool outcomes. Production
App launch mounting and authorization remain open parts of slice 14.

## Background

The [management checkpoint](2026-09-18-app-management-api.md) pinned discovery and argument schemas.
Checking only immediate responses would leave asynchronous task results outside the App contract.
Rejecting invalid output must also preserve the durable boundary of a write that may have taken effect.

## Scope

The common journal validation seam, immutable tool attribution on sealed task permits, optional
validator propagation through proxy exchanges/subscriptions, and focused protocol/store regressions.
The [journal contract](../features/mcp-operation-journal.md#integration-owned-result-validation)
defines the completed-result boundary.

## Key Decisions

- Resolve the schema's tool identity from the original operation, not task IDs or upstream metadata.
- Separate deferred task/input-required receipts from final output. Legacy status-only completion
  still requires `tasks/result`; modern query/subscription completion validates its nested result.
- Validate native tool errors too, allowing an integration to bound their payload while applying
  success schemas only to successful results. Fixed error categories never echo invalid bodies.
- Invalid immediate/continued/batch output retains an unknown write outcome. Existing replay policy
  applies; output validation cannot silently grant a retry.
- A mixed batch commits valid neighbors even when invalid output appears earlier in the same frame.
  The invalid frame is not delivered. A failed task-result observation does not settle the original
  accepted task; a later valid observation can reconcile without another tool send.
- Keep a single proxy and journal. The optional validator is binding configuration; Mem defaults
  remain unchanged. App-specific total-result bounds and schema adapter wiring are still required.

## Validation

All 118 MCP tests and 48 affected database journal tests pass. All-target MCP/DB Clippy passes with
warnings denied. Five new workflow tests cover direct/native-error results, both mixed-batch orders,
input-required continuation, legacy/modern task lookup, and modern task notifications. They assert
durable outcomes, unchanged valid responses, original tool names, and actual upstream send counts.

The initial test incorrectly treated all identical future writes as forbidden after a known result.
The existing contract permits an intentional new request after success. The corrected assertion
targets unknown output and proves it cannot resend; production replay policy was not weakened.

```bash
cargo +1.96.0 test -p agenthub-mcp --lib --locked --offline
cargo +1.96.0 test -p agenthub-db mcp_operations --locked --offline
cargo +1.96.0 clippy -p agenthub-mcp -p agenthub-db --all-targets --locked --offline -- -D warnings
cargo +1.96.0 fmt --all --check
```

## Follow-Ups

Wire App manifests into the production resolver, launch snapshot, shared proxy request/stream
admission, Card projection, and activation trace. Prove the registered-App path through an actual
controller/CLI/HTTP fixture before publishing slice 14. Slices 15–18 remain part of the active goal.
