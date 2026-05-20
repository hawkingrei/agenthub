# Team Mailbox Phase 2: Ownership And Task Links

## Summary

Completed Team mailbox phase 2 by adding topic ownership leases and durable mailbox message to
task links on top of the phase 1 triage/disposition model.

## Background

Phase 1 separated mailbox delivery state from handling disposition so actors could triage mailbox
items as `ignored`, `watching`, `claimed`, `completed`, or `released`.

That still left two gaps for real Team coordination:

- multiple actors could race to act on the same longer-lived mailbox topic without a durable owner;
- mailbox evidence could not be attached to a canonical Team task without relying on free-form note
  text or payload conventions.

The stable contract for these gaps lives in
`docs/features/team-mailbox-intake-and-ownership.md`.

## Scope

- Added thread/topic claim persistence with lease-based ownership.
- Added durable mailbox message to task link persistence.
- Extended actor mailbox reads, internal gRPC, and actor CLI surfaces to expose ownership and task
  link state.
- Updated Team mailbox skill and managed skill text so agents use `triage` and `task-link`
  deliberately.

## Key Decisions

- Kept transport delivery state (`pending`, `delivered`, `dead_letter`) separate from handling
  disposition and topic ownership.
- Modeled ownership as topic-scoped lease records in `team_actor_thread_claims` rather than adding
  more overload to per-message status fields.
- Modeled task linkage as explicit records in `team_actor_message_links` so durable associations are
  queryable without parsing free-form notes.
- Reused `actor triage --disposition claim|release|complete` as the ownership transition path
  instead of introducing another partially overlapping command family.
- Added `agenthub actor task-link` for explicit durable task association, and extended internal gRPC
  with `LinkActorMessageTask`.
- Kept the separate `actor inbox --include-delivered` regression out of scope for this rollout
  because it is being repaired in another PR.

## Validation

Executed:

```bash
cargo fmt --all
cargo check -p agenthub --tests
cargo test -p agenthub-team-actor -- --nocapture
cargo test -p agenthub actor_mailbox_service_ -- --nocapture
cargo test -p agenthub internal_grpc_mailbox_ -- --nocapture
cargo test -p agenthub parse_task_link_accepts_relation_and_message_ids -- --nocapture
cargo test -p agenthub receive_actor_inbox_consumes_pending_messages -- --nocapture
cargo test -p agenthub-managed-skills -- --nocapture
```

## Follow-Ups

- Phase 3 still remains: canonical inbound-envelope normalization for future trigger/human/webhook
  channels, plus runtime enforcement for `requires_user_visible_reply`.
- Operator-visible Team UI still needs first-class visibility for unresolved reply obligations and
  manual takeover/observe state on claimed mailbox topics.
