# Native Opening Reconciliation

## Summary

Repair a lost outer conversation binding when its native opening ACK was already
committed. Recovery uses the previous exact local launch's closed event owner and
verified executor retirement. It does not retry creation or execution.

## Background

Opening spans the event database and the control database. A daemon can exit after
committing the accepted create/resume receipt and before binding that conversation
to its activation. Treating this known result as permanently uncertain would block
an otherwise recoverable conversation.

## Scope

- Bounded accepted-opening evidence from a closed event owner.
- Idempotent binding repair under the current guarded reservation.
- Managed startup wiring before normal configuration-fenced opening.
- Focused storage and manager regression coverage; no schema or wire change.

## Key Decisions

Only one accepted opening and one matching stream can establish identity. Prepared,
sent, rejected, missing or conflicting receipts cannot establish it. The evidence
type cannot be constructed or deserialized by external callers. Each repair checks
the old local launch, actor/Team association, generation and cleanup event, as well
as the current lease and membership. Resume never substitutes a different native
conversation. A repeated identical repair is harmless and adds no duplicate trace.

The manager routes evidence through the actor's event database; it never scans native
checkpoint directories. Repair leaves old event history closed, does not restore
credentials and sends no provider request. Ordinary opening, source registration,
reentry review and approval controls still apply afterward.

Contract: [direct runtime integration](../features/rara-direct-integration.md).

## Validation

```bash
cargo test --locked -p agenthub-db --lib runtime_events::
cargo test --locked -p agenthub-db --lib native_session_tests::
cargo test --locked -p agenthub --lib loop_launch::tests::native::
cargo clippy --locked -p agenthub -p agenthub-db --lib --tests -- -D warnings
cargo fmt --all --check
```

The storage cases cover accepted create/resume, database reopen, duplicate repair,
unknown outcomes, conflicting receipts and streams, old-generation evidence,
missing retirement, configuration mismatch and revoked authority. The managed case
retains a real local fixture process's receipts, reconstructs the binding crash window
and distinguishes committed acceptance from an unknown result without another input.
It exercises the recovery component below the still-closed Resume preflight gate;
it is not assembled native restart or configured-provider qualification.

Results: 33 runtime event cases, 6 native binding cases and all 16 affected managed
native activation cases pass. Root/DB library-and-test Clippy passes with warnings
denied. Formatting, whitespace and all 119 local links in the changed documentation
pass.

## Follow-Ups

Truly unknown openings remain blocked and may be explicitly replaced using fresh
policy only after verified cleanup. Complete assembled native process restart and
browser acceptance, standalone continuity and installed/configured-provider validation
before enabling Resume. Existing upstream publication authorization remains pending.
