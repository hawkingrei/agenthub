# Actor Send Direct Target Validation

## Summary

- reject direct `agenthub actor send --to-actor-id ...` targets that are not canonical Team `spec.members[].member_id` values
- keep human mailbox targets (`user`, `user:<id>`) valid
- fail fast on role aliases such as `leader` before they are persisted into `team_actor_messages`

## Background

Mailbox investigation on Team run `737faf97-31c8-4ad2-8669-7b124a720541` showed that message `2148` was not lost. It was persisted as:

- `from_actor_id = c319f933-1358-4418-a111-872304052422` (worker-1)
- `to_actor_id = leader`
- `status = pending`

The same Team definition uses `595d1ae8-fcbd-4111-b5c7-d446a12c044b` as the canonical leader `member_id`, and messages addressed to that UUID were delivered normally. The direct send path accepted the role alias `leader` verbatim and wrote it to `team_actor_messages`, which left the record permanently unread by the actual leader mailbox consumer.

## Implementation

- add direct mailbox target validation in `src/actor_cli/execute.rs`
- load Team context for the resolved run before direct `actor send`
- accept only:
  - Team `spec.members[].member_id`
  - human mailbox aliases `user` / `user:<id>`
- reject role aliases with a targeted hint to the canonical `member_id`

## Validation

- `cargo test -p agenthub validate_direct_mailbox_target_ -- --nocapture`
- `cargo test -p agenthub parse_send_generates_default_idempotency_key -- --nocapture`
- `git diff --check`
