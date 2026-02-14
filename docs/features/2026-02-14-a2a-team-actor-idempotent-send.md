# A2A Team Actor Idempotent Send

## Summary

Add a complete idempotent send strategy for Team actor mailbox:

- explicit `idempotency_key` support,
- default auto-generated idempotency keys for actor CLI send calls,
- conflict rejection when the same key is reused with different payload/routing,
- and an explicit duplicate-delivery escape hatch.

## Background

The local Team actor mailbox already supports durable send/inbox/ack flow, but
retrying `messages/send` could create duplicate rows and duplicate sent events.
Scheduler/orchestrator retry paths need exactly-once semantics at the API
boundary while keeping transport delivery at-least-once. At the same time,
agents need a safe default so retrying `actor send` does not accidentally
create duplicates.

## Scope

- `crates/agenthub-team-actor/src/mailbox.rs`
- `crates/agenthub-team-actor/src/idempotency.rs`
- `src/team/manager/mailbox.rs`
- `src/api/teams.rs`
- `src/actor_cli.rs`
- `crates/agenthub-acp/src/lib.rs`
- `crates/agenthub-acp/src/actor_runtime_skill.rs`
- `src/db.rs`
- `src/team/manager/tests.rs`
- `src/api/teams/tests.rs`
- `src/api/teams/tests_core.rs`
- `src/api/teams/tests_router.rs`
- `docs/todo.md`

## Key Decisions

- Extend send command with optional `idempotency_key`.
- Add shared canonical fingerprint helpers in `agenthub-team-actor` so client
  and server use the same deterministic hashing rules.
- Add partial unique index for mailbox dedupe:
  - `UNIQUE(run_id, from_actor_id, idempotency_key)`
  - only applies when `idempotency_key IS NOT NULL`.
- Keep ActorMailbox event semantics explicit:
  - store returns `{ message, created }`,
  - `actor_message_sent` is emitted only when `created=true`.
- Add strict conflict protection for reused keys:
  - when an existing key is hit, server compares message fingerprints,
  - if fingerprints differ, API returns `409 conflict`.
- Keep API validation strict:
  - `idempotency_key` must be non-empty when present,
  - max length is 128 chars.
- Extend actor CLI:
  - default `actor send` auto-generates `auto:v1:<hash>` when key is omitted,
  - `--allow-duplicate` disables default idempotency for intentional repeated delivery,
  - `--idempotency-key` + `--allow-duplicate` is rejected.
- Update actor runtime skill text so agents know:
  - default send is idempotent,
  - payload changes under the same key will be rejected,
  - `--allow-duplicate` is the explicit opt-out.

## Validation

```bash
cargo test -p agenthub-team-actor
cargo test actor_message_send_is_idempotent_by_key -- --nocapture
cargo test actor_message_send_rejects_mismatched_payload_for_same_idempotency_key -- --nocapture
cargo test team_run_messages_api_supports_idempotency_key -- --nocapture
cargo test teams_router_http_contract -- --nocapture
```

## Follow-ups

- Decide whether idempotency dedupe scope should include `to_actor_id` in later
  cross-team routing phases.
