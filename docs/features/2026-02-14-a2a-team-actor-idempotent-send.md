# A2A Team Actor Idempotent Send

## Summary

Add `idempotency_key` support for Team actor mailbox send so orchestrator retries
can safely reuse an existing message record and avoid duplicate
`actor_message_sent` events.

## Background

The local Team actor mailbox already supports durable send/inbox/ack flow, but
retrying `messages/send` could create duplicate rows and duplicate sent events.
Scheduler/orchestrator retry paths need exactly-once semantics at the API
boundary while keeping transport delivery at-least-once.

## Scope

- `crates/agenthub-team-actor/src/mailbox.rs`
- `src/team/manager/mailbox.rs`
- `src/api/teams.rs`
- `src/actor_cli.rs`
- `src/db.rs`
- `src/team/manager/tests.rs`
- `src/api/teams/tests.rs`
- `src/api/teams/tests_core.rs`
- `docs/todo.md`

## Key Decisions

- Extend send command with optional `idempotency_key`.
- Add partial unique index for mailbox dedupe:
  - `UNIQUE(run_id, from_actor_id, idempotency_key)`
  - only applies when `idempotency_key IS NOT NULL`.
- Keep ActorMailbox event semantics explicit:
  - store returns `{ message, created }`,
  - `actor_message_sent` is emitted only when `created=true`.
- Keep API validation strict:
  - `idempotency_key` must be non-empty when present,
  - max length is 128 chars.
- Extend actor CLI:
  - `agenthub actor send --idempotency-key <key>`.

## Validation

```bash
cargo test -p agenthub-team-actor
cargo test actor_message_send_is_idempotent_by_key -- --nocapture
cargo test team_run_messages_api_supports_idempotency_key -- --nocapture
```

## Follow-ups

- Verify end-to-end orchestrator retry path sends stable idempotency keys for
  Team actor messages.
- Decide whether idempotency dedupe scope should include `to_actor_id` in later
  cross-team routing phases.
