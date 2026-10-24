# Native Conversation Binding

## Summary

Native loop launches now record a durable conversation opening before dispatch and bind the
accepted native identity after committing its receipt and runtime stream. This is the outer
ownership foundation for restart continuity, not completion of cross-process recovery.

## Background

Every launch previously created a new native session. The activation configuration digest includes
temporary activation identity and cannot select a historical conversation safely. Reusing a stored
provider id alone would also lack proof that its old executor and descendants had retired.

## Scope

- Add an actor/Team-scoped conversation binding with exact activation, generation and local launch.
- Derive a stable configuration digest from canonical workspace, Card/role, prompt, tool bindings,
  native launch configuration and negotiated provider/model; exclude activation work and credentials.
- Add a typed resume request and durable receipt validation for the exact requested conversation.
- Keep resume preflight closed while pending-input and interrupted-operation recovery are integrated.

## Key Decisions

- Opening intent and binding commits use SQLite FULL synchronization before dependent external work.
- Replacement requires the prior activation/generation's `cleanup_verified` event. An expired lease,
  ended session or successful protocol shutdown cannot substitute for supervised descendant cleanup.
- First use may create a conversation. Known incompatible or ambiguous resume state never falls back
  to create; explicit fresh policy requires the same prior-cleanup proof.
- A resume request has no live session provenance on the wire. Its durable intent nevertheless names
  the expected native session, and a different ACK cannot establish runtime stream ownership.
- Event cursors remain per runtime. The ACK opens a zero-cursor stream before event consumption;
  only committed contiguous events advance it.
- The stable binding grants no historical permission, tool authority or approval callback ownership.

## Validation

Focused validation commands:

```bash
cargo test --locked -p agenthub-rara --lib
cargo test --locked -p agenthub-db --lib
cargo test --locked -p agenthub --lib continuity_digest
cargo test --locked -p agenthub --lib native_
cargo test --locked -p agenthub --lib agent::manager::rara::tests::
cargo clippy --locked -p agenthub --lib --tests -- -D warnings
cargo fmt --all --check
```

Coverage includes initial create, repeated-opening rejection, migration/reopen, exact cleanup
generation, configuration mismatch, ambiguous opening, explicit fresh replacement, stale owner and
late ACK rejection, plus exact resume identity and zero-cursor event ownership. The managed loop
fixture checks that consecutive fresh activations receive different native/local identities while
retaining the same stable configuration digest and mailbox identity.

The rebuilt CLI supports the managed fixtures: 22 native-path cases and 34 manager transport
cases pass, along with the stable-digest regression. Existing opt-in process cases remain ignored
by these filters. The fixtures require permission to bind local sockets; the initial restricted
run failed during fixture setup, and the permitted run completed without test failures.

## Follow-Ups

Consume restored waiting/recovery states under current authority, reconcile uncertain decisions,
reconstruct current-owner approval UI, and qualify actual process restart with the assembled
configured provider before removing resume preflight. Standalone continuity remains unfinished.
Upstream publication still requires the existing destination-specific authorization.

Contract: [direct runtime integration](../features/rara-direct-integration.md).
