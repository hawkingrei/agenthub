# Native Semantic Outcomes

## Summary

Native activation entries use the pinned runtime's bounded semantic guard. A normally
finished decline can record `no_actionable_work` or an atomic clarification question,
`waiting/input` outcome and one-shot reply registration through existing canonical stores.

## Background

The reopened native rollout requires typed semantic decisions. A normal provider turn
finish or a reason string cannot independently establish a canonical loop outcome.
The upstream producer is locally qualified at
`5585583a6674cb0d6ac36d5be98744dd75589871`; its publication remains a delivery gate.

## Scope

- Negotiate the guarded-input method and semantic event family before session creation.
- Supply bounded role/Card context and exact addressed task/message sources to the guard.
- Persist strict decision events in scoped history, keeping decision text out of telemetry.
- Correlate decisions with ordered terminal events and the durable entry request ACK.
- Compose the existing finish writer, conversation writer and reply scheduler in one transaction.
- Preserve ordinary ACP delivery, interactive native input and the `LoopOutcome` wire format.

## Key Decisions

- `native-loop-v4` records the changed entry contract in the launch digest.
- Only the exact activation entry request can consume its configured guard context.
- A unique runtime-provenance decline must precede normal completion of its admitted turn.
  Foreign, duplicate, late or conflicting events, cancellation and failed delivery cannot
  manufacture a finish. Compatible/unavailable classification continues ordinary execution.
- An existing actor finish wins even when its outcome kind equals the proposed guard result.
- Clarification uses the single pinned task conversation or the shared Team conversation.
  Its new root is the exact reply target. Canonical publication, finish and the observation
  latch commit together; no new queue or schema migration is introduced.
- A reply can arrive before executor cleanup. Its durable activation must wait for the
  existing writer reservation to be released after verified cleanup.
- A crash before the control-store transaction commits leaves an interrupted activation
  with scoped runtime history. Recovery does not fabricate an outcome from that history.

## Validation

The protocol fixture was captured from the independently preserved native executable:

- Revision: `5585583a6674cb0d6ac36d5be98744dd75589871`
- SHA-256: `dcf4639b05a51f274a210eb950e592c85564c554544370c3145e0e7708b7adc1`
- Six actual handshake/create/shutdown frames; identifiers normalized in the fixture.

Focused validation commands:

```bash
cargo test --locked -p agenthub-rara --lib
cargo test --locked -p agenthub-db guarded_prompt_receipt
cargo test --locked -p agenthub-db loop_runtime::tests::lifecycle_tests
cargo test --locked --lib native_semantic
cargo test --locked --lib agent::manager::loop_launch::tests::native::
cargo test --locked --lib native_loop_process_ -- --ignored --nocapture
cargo test --locked --lib controlled_proxy_sources_enforce_scope_revocation_and_uncertainty -- --ignored --nocapture
cargo clippy --locked --lib --tests -- -D warnings
cargo fmt --all --check
```

The protocol suite reports 52 passing tests, the guarded receipt regression passes, and
14 existing lifecycle tests preserve the finish contract. Four canonical transaction tests
cover rollback, stale ownership, authoritative finish and SQLite reopen before reply arrival.
Eleven fake-peer regressions cover source binding, pending input, declines and late/foreign ACKs.
Actual native-process scenarios cover all three semantic decisions, invalid-response fallback,
the dispatch/report/acceptance cycle with child isolation, and controlled App/Mem proxies with
revocation and uncertain-write handling. These local HTTP provider fixtures do not establish
installed/configured-provider acceptance. After the event-order review, the 11 fake-peer
regressions, both actual-process tests and the controlled-proxy parent pass again. Root
lib/tests Clippy denies warnings; formatting and 78 local documentation links are clean.

## Follow-Ups

- Complete applicable CI for this adapter change.
- Publish the upstream prerequisites before treating the pinned adapter as deliverable.
- Continue cross-process continuity, durable approvals and installed-provider acceptance
  in the [active rollout](../todo.md).
- Stable behavior is specified in [direct runtime integration](../features/rara-direct-integration.md).
