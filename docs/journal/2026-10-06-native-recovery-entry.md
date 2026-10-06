# Native Recovery Entry

## Summary

Restored native waits receive current loop sources and a tool-free entry check before
their callbacks become actionable. The consumer binds the typed result to its exact
request, target and durable event prefix. Interrupted recovery has an authenticated
explicit reconciliation API; applying it never starts or replays a turn.

## Background

The native checkpoint producer can restore pending interactions before this activation
has registered its context. Publishing a live callback immediately could let an old
answer execute ahead of current entry checks. An ordinary prompt also cannot enter a
blocked session without consuming or discarding its wait.

## Scope

- Strict recovery status, bounded decision receipts, reentry controls and event decoding.
- Current source registration while waiting or blocked, followed by correlated read-only
  classification and delayed approval publication.
- Canonical mismatch/clarification outcomes without a synthetic worker turn.
- Live executor verification and operation ownership for input and approval admission.
- An operator reconciliation endpoint fenced by local launch, runtime, conversation and
  recovery token. Repeated confirmed resolution is idempotent and does not send input.
- An authenticated current-state query and durable-state refresh before subsequent idle
  input, so cancellation cannot leave standalone sessions without an actionable token.

## Key Decisions

Entry events may precede ACK delivery. They are persisted as evidence but cannot open
callbacks until the matching accepted receipt and event prefix commit. Foreign,
duplicate, missing or out-of-prefix results fail the runtime. Advisory classifier
unavailability preserves the existing behavior and never resolves uncertain effects.

Permission answers preserve the operator's selected choice separately from execution
admission. A revoked membership, expired lease or stale owner cannot dispatch an answer.
The existing operation guard prevents cleanup from releasing an admitted generation
while a control settles. Scope is checked again after waiting for the input/event gate.

The fast restored-approval regression exposed an existing shared permission publication
race: a reviewer could mark the durable row responded before its live callback existed.
Request creation now holds the response-delivery mutex across row publication, session
status update and callback registration. A dedicated regression fails before the fix;
no timing delay or test-only bypass is added to the runtime.

The protocol fixture is captured from local producer commit
`178dfecf6599ad536f53f5f207d37d547b510a6b`, executable SHA-256
`70c42d4622ded1c04103a0d7a2087dbe06083930d0e0646bb80c6f28f8a32dac`.
Package version alone does not establish these capabilities. The source contract is
`native-loop-v5`; resume preflight remains closed until assembled acceptance completes.

Contract: [direct runtime integration](../features/rara-direct-integration.md).

## Validation

```bash
cargo test --locked -p agenthub-rara --lib
cargo test --locked -p agenthub-db --lib runtime_events::tests::requests
cargo test --locked -p agenthub-acp --lib permission
cargo test --locked -p agenthub --lib loop_launch::tests::native::
cargo test --locked -p agenthub --lib agent::manager::rara::tests::
cargo clippy --locked -p agenthub -p agenthub-acp --lib --tests -- -D warnings
cargo fmt --all --check
```

Focused cases cover recovery payload bounds and ambiguous records, trusted event origin,
private history versus telemetry, exact blocked targets, event-before-ACK callback gating,
canonical declines, foreign result rejection, revoked membership, API authorization and
explicit nonreplaying reconciliation. Native fixtures require local socket permission.

Results: 57 protocol cases, 15 receipt cases, 14 shared permission cases, 15 managed
loop cases and 35 manager transport/input cases pass. The dedicated shared permission
publication regression fails against the previous implementation and passes with the
delivery lock. The final full managed group also passes after that fix.

Coverage CI exposed a fixture cancellation bug in that regression: timing out a
pool query could drop the only in-memory SQLite connection and erase its schema.
The observer now checks its deadline between completed queries. It retains the
publication assertion and does not alter production synchronization.
The full ACP library passes 63 cases after this fixture correction, and the
publication regression passes 20 consecutive repetitions. ACP Clippy and formatting
also pass; the refreshed coverage run remains the remote acceptance check.

The actual pinned child passes transport/source/shutdown and managed user/plan/shell
approval round trips (both allow and deny choices) against a local model fixture. These
two opt-in cases were selected explicitly with `AGENTHUB_RARA_TEST_BINARY`; they do not
claim assembled process-restart or external configured-provider acceptance. The ordinary
protocol/manager filters retain their existing opt-in exclusions.

Root and ACP library/test Clippy pass with warnings denied. Formatting and whitespace
checks pass, and all 112 local links in the changed documentation resolve.

## Follow-Ups

Browser actions are recorded in [native recovery controls](2026-10-07-native-recovery-controls.md).
Qualify the assembled actual-process resume path, reconcile ambiguous conversation openings,
and finish standalone continuity plus installed/
configured-provider acceptance. Upstream publication still needs its existing exact-
destination authorization. None of these remaining milestones is closed by this checkpoint.
