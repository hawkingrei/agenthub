# Signed App Intake

## Summary

Signed App notifications now reach the canonical activation intake through one durable transaction.
Rejected logical events leave bounded audit summaries without retaining raw payloads or signatures.

## Background

The configuration checkpoint separated event route authority from tools and provisioned independent
signing-key references. Event delivery must preserve that authority under revocation, retries,
concurrent publishers, database failures, and storms.

## Scope

Signature verification, bounded HTTP notifications, event/trigger receipts, monotonic cursors,
durable accepted-event budgets, safe source attribution, and owner-only rejection inspection.

## Key Decisions

- Bind the protocol domain, registered App ID, key version, timestamp, and exact body with HMAC-SHA256.
- Recheck key and route authority under the write lock before even returning a duplicate receipt.
- Reuse loop intake inside a savepoint; rollback cursor, budgets, and trigger together on failure.
- Preserve semantic notification identity across JSON formatting and key rotation retries.
- Count only newly accepted logical events against App/actor/Team windows. Duplicate observations
  remain in the ordinary loop history. Clock rollback cannot reset a budget.
- Bound denial storage to five fixed codes per App; no per-request unauthenticated rows.

## Validation

Seven intake storage tests pass: concurrent duplicate serialization, identity/cursor/route rejection,
key rotation/revocation, injected receipt-write rollback, disabled/full intake recovery, App/actor/Team
budgets, and reopen without reactivation.

Root App workflows pass 19 tests, including the independent signature vector and isolated signed HTTP
fixture; two ignored child fixtures execute through their parents. Static API authorization guards
pass 3 tests. The domain suite passes 15 tests. Root/domain/DB all-target Clippy passes with warnings
denied. The final indexed receipt/reopen follow-up passes all 186 DB tests; its DB Clippy follow-up
also passes. Formatting, whitespace, and 29 focused local documentation links pass.

```bash
cargo +1.96.0 test -p agenthub-db --lib app_registry::tests::event_intake
cargo +1.96.0 test -p agenthub --lib ::apps::
cargo +1.96.0 test -p agenthub --lib api::authz::
cargo +1.96.0 test -p agenthub-agent-domain -p agenthub-db --lib
cargo +1.96.0 clippy -p agenthub -p agenthub-agent-domain -p agenthub-db --all-targets -- -D warnings
```

## Follow-Ups

Connect standing App-event conditions and doctor/context presentation, then publish the full
slice with exact-head CI. The canonical runtime contract is
[signed App event intake](../features/app-event-ingress.md).
