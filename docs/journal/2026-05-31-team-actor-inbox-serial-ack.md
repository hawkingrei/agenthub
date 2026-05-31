# Team Actor Inbox Serial Ack

## Summary

Actor CLI inbox receive now accepts pending mailbox messages serially instead of acking and
triaging multiple messages concurrently. This keeps the actor-facing receive path stable when the
runtime peer is backed by SQLite and ack/triage performs multiple writes.

## Background

The receive helper previously used bounded concurrency for pending-message ack. That preserved
output ordering after sorting, but concurrent ack/triage mutations can still collide with
SQLite-backed runtime control and surface a generic internal mailbox failure.

## Scope

- Process pending inbox messages one at a time during `actor receive`.
- Keep response ordering unchanged for multiple pending messages.
- Preserve the existing claim-first triage behavior and conflict fallback to `watching`.
- Surface generic internal mailbox error details instead of replacing them with an opaque message.

## Key Decisions

- Serial receive is preferred over retrying write-lock failures because actor receive is an
  interactive control path and correctness is more important than parallel ack throughput.
- The existing not-found behavior remains unchanged: if a message disappears before ack, receive
  keeps the original item in the response.
- Read-only database errors keep their explicit operator-facing message; other internal failures
now include the causal error text for diagnosis.

## Validation

Focused checks:

```bash
cargo test -p agenthub receive_actor_inbox -- --nocapture
cargo test -p agenthub map_actor_service_error -- --nocapture
cargo test -p agenthub internal_grpc_team_context_and_task_controls_are_wire_compatible -- --nocapture
```

## Follow-Ups

- Continue the mailbox phase 3 terminal-outcome invariant audit from `docs/todo.md`.
