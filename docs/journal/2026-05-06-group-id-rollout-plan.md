# Group Id Rollout Plan

## Summary

The distributed metadata specs now define the physical rollout order for `group_id` without
prematurely enforcing multi-tenant routing.

## Background

`group_id` is the intended long-term trust and routing boundary for distributed nodes, messages, and
future multi-tenant work. The archive schema already preserves optional `group_id`, but the live
authority rows do not consistently carry it yet.

## Scope

- Define the authority-first rollout sequence for `group_id`.
- Keep node-local mirrors and message projections subordinate to `main` authority.
- Make routing enforcement explicitly depend on populated node registry and message authority rows.

## Key Decisions

- `group_id` should start on main-owned control-plane authority rows, then propagate into message
  authority rows and projections.
- Existing `owner_user_id` is only a compatibility boundary for single-user installations. It is not
  the final group identity model.
- Missing `group_id` should be treated as `unknown`, not as permission to route across groups.
- Gossip may carry group membership only after registry authority rows define it.

## Validation

This is a docs-only planning slice. The follow-up implementation phases should add focused migration
tests for each physical schema step and keep the existing relay tests green:

```bash
cargo test remote_actor_messages_relay_success_marks_message_delivered -- --nocapture
cargo test bidirectional_actor_grpc_pipeline_relays_seeded_messages_between_in_process_states -- --nocapture
```

## Follow-Ups

- Add the first physical `group_id` authority column to the main-owned node registry surface.
- Backfill existing single-user installations into one default group boundary.
- Propagate `group_id` into Team/message authority rows before enabling routing enforcement.
