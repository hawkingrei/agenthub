# Team Runtime Delivery Receipts

## Summary

Replaced best-effort Team runtime hints with durable, restart-recoverable delivery receipts while
preserving mailbox acknowledgement and handling as separate actor-owned state.

## Background

Direct actor messages, coordinator mentions, and permission reviews wrote durable mailbox rows but
then attempted a one-shot runtime prompt. A transport error or daemon restart after the mailbox write
could strand the prompt indefinitely. Reusing `team_actor_messages.status` would have collapsed
runtime submission into explicit actor acknowledgement and broken the mailbox ownership contract.

## Scope

- Added `team_runtime_delivery_receipts` with stable IDs, retry state, leases, session attribution,
  and a due index.
- Persisted receipts before direct-message, coordinator-mention, and permission-review hint delivery.
- Added an immediate dispatcher and a startup retry worker using the same claim path.
- Forwarded the stable delivery ID into local and remote agent input submission.
- Added attempt fencing so expired workers cannot acknowledge or retry a newer lease.
- Kept unread-summary hints reconstructable, while assigning deterministic event IDs.

## Key Decisions

- Kept mailbox delivery/acknowledgement independent from runtime input submission.
- Defined runtime delivery as input-transport acceptance by a specific session, not actor handling.
- Used at-least-once delivery because a process can crash between transport acceptance and database
  acknowledgement.
- Allowed the current runtime's active run to differ from the mailbox run. Permission-review routing
  can target a running reviewer while storing the request in a shared mailbox run.
- Applied retry delay to offline runtimes so a bounded due batch cannot be permanently occupied by
  old unavailable actors.

## Validation Coverage

The implementation is covered by:

```bash
cargo fmt --all
cargo check -p agenthub --lib
cargo clippy -p agenthub --lib -- -D warnings
cargo test -p agenthub-db --lib -- --nocapture
cargo test -p agenthub --lib runtime_delivery_receipts -- --nocapture
cargo test -p agenthub --lib team::permission_review::tests -- --nocapture
cargo test -p agenthub --lib team_run_messages_api_chat_type_hints_repeat_while_other_types_still_suppress -- --nocapture
cargo test -p agenthub --lib
bazel build //crates/agenthub-db:all
bazel build //:agenthub_lib
```

The focused receipt cases assert retry with the same delivery ID after worker reconstruction,
idempotent acknowledgement, expired-lease recovery, and rejection of stale attempt updates. The
permission-review cases retain shared-mailbox-run delivery, and the Team API case exposes stable
delivery IDs in diagnostic run events.

## Follow-Ups

- Define retention or abandonment for receipts whose target actor never becomes available again.
- Move this worker and the other daemon background workers into the unified cancellation-aware task
  group.
- Preserve exact-head Cargo, Bazel, and cross-platform CI evidence on the delivery PR.
