# Signed App Intake

## Summary

Signed App notifications now reach the canonical activation intake through one durable transaction.
Rejected logical events leave bounded audit summaries without retaining raw payloads or signatures.
Standing conditions reuse canonical scheduling and retain safe event attribution in context and history.

## Background

The configuration checkpoint separated event route authority from tools and provisioned independent
signing-key references. Event delivery must preserve that authority under revocation, retries,
concurrent publishers, database failures, and storms.

## Scope

Signature verification, bounded HTTP notifications, event/trigger receipts, monotonic cursors,
durable accepted-event budgets, safe source attribution, owner-only rejection inspection, standing
event conditions, doctor output, actor CLI help, and web history presentation.

## Key Decisions

- Bind the protocol domain, registered App ID, key version, timestamp, and exact body with HMAC-SHA256.
- Recheck key and route authority under the write lock before even returning a duplicate receipt.
- Reuse loop intake inside a savepoint; rollback cursor, budgets, and trigger together on failure.
- Preserve semantic notification identity across JSON formatting and key rotation retries.
- Count only newly accepted logical events against App/actor/Team windows. Duplicate observations
  remain in the ordinary loop history. Clock rollback cannot reset a budget.
- Bound denial storage to five fixed codes per App; no per-request unauthenticated rows.
- Install watches and initial receipt observations under one write lock; accepted events update
  matching watches atomically, and duplicate delivery never relatches them.
- Pin each watch to its approved route revision. Authority changes retire idle and completed watches
  through ordinary source revocation; key rotation and unchanged permissions preserve watches.
- Preserve the first matched event's original manifest version while a firing records its coalesced
  cursor range. Direct notification and condition firings remain distinct canonical sources.

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

### Standing-condition integration

- Eight new storage workflows cover concurrent registration/event arrival, duplicate delivery,
  once/repeat cursor coalescing, history catch-up and reopen, capacity retries, injected observation
  rollback, cross-member isolation, all App authority changes, and task/origin cancellation.
- The full domain/DB suites pass 16/194 tests. The final App wait-metric addition passes the eight
  event-condition workflows and six existing metric tests; App wait counts include dormant watches.
- A signed control-RPC workflow registers approved event intent, finishes the original activation,
  admits its successor, and reads original event attribution through context/source APIs. It rejects
  tool-only authority and the prior execution credential, and retains facts after route revocation.
- Six existing CLI/HTTP/RPC/provider scheduling regressions pass. The App selection passes 19 tests
  plus two child fixtures executed by their parents; API authorization guards pass 3 tests.
- Diagnostics pass 15 tests, including safe event rendering after exit and read-only database reopen.
  Root/domain/DB/diagnostics all-target Clippy passes with warnings denied.
- Focused web history/member tests pass 15 cases; typecheck, lint, and production build pass. Two
  Playwright loop workflows pass using cached Chrome 149 through an external temporary configuration;
  the default Playwright headless-shell revision is not installed on this host.
- Actual Chrome DevTools MCP 1.9.0 before/after inspection shows the prior timer mislabel replaced by
  the App event wait and original event ID/class/cursor/version beside a revoked source. At 390px the
  document stays 390px wide and event attribution wraps. Evidence is retained locally as
  `/tmp/agenthub-loop-pr15-event-{before,after}.{txt,png}` and `event-mobile.png` with the same prefix.
  The synthetic fixture's unrelated `/api/teams/prompt_defaults` request returns 404; the event
  history and schedule requests succeed. All temporary browser and fixture processes were stopped.

```bash
cargo +1.96.0 test -p agenthub-db --lib event_scheduling
cargo +1.96.0 test -p agenthub-db --lib metrics
cargo +1.96.0 test -p agenthub --lib app_event_schedule_rpc
cargo +1.96.0 test -p agenthub --lib loop_schedule
cargo +1.96.0 test -p agenthub-diagnostics --lib
cargo +1.96.0 clippy -p agenthub -p agenthub-db -p agenthub-agent-domain -p agenthub-diagnostics --all-targets -- -D warnings
cd web
npm exec -- vitest run src/pages/team/loop_activation_history.test.tsx src/pages/team/team_loop_member_panel.test.tsx
npm exec -- tsc --noEmit
npm run lint
npm run build
npm exec -- playwright test tests/e2e/team_loop.e2e.ts --project=chromium --workers=1
```

## Follow-Ups

Publish the complete slice and validate exact-head CI. The canonical runtime contract is
[signed App event intake](../features/app-event-ingress.md).
