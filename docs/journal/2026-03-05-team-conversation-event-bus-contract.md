# 2026-03-05 Team Conversation Event Bus Contract

## Context

Team collaboration needed a clearer split between:

- human-facing realtime conversation flow, and
- execution-grade mailbox/actor reliability semantics.

We aligned on a conversation-first model where users should not be forced to provide
execution-only fields like `run_id`.

## Decisions

1. Event bus is the communication carrier, not the state authority.
- Chat/timeline fan-out uses event bus.
- Authoritative records stay in `main` DB with persist-first outbox relay.

2. Mailbox remains execution command authority.
- `assignment`, `approval`, `step_action`, execution results remain mailbox-backed.
- Event bus can mirror visibility events but cannot replace mailbox ack semantics.

3. Identity and lifecycle mapping is explicit.
- `conversation_id` (required, user-facing scope).
- `task_id` (leader-defined internal work item).
- `run_id` (execution instance, generated when execution starts).
- `correlation_id` (intent-chain linkage across conversation/mailbox/run events).

4. Input normalization is server responsibility.
- User/agent chat input may omit `run_id` and `from_actor_id`.
- Backend derives sender identity from session and resolves recipients from `@member_id`.

## Documentation Changes

- Added canonical spec:
  - `docs/features/team-conversation-event-bus.md`
- Synced related feature docs:
  - `docs/features/teams-collaboration-playbook.md`
  - `docs/features/agents-teams.md`
  - `docs/features/actor-foundation.md`
  - `docs/features/backend-runtime-logic.md`
- Updated canonical feature index:
  - `docs/features/README.md`
- Added verification backlog item:
  - `docs/todo.md`

## Result

Team docs now clearly separate:

- conversation/event-bus realtime carrier responsibilities, and
- mailbox/actor execution reliability responsibilities.

This keeps chat UX lightweight while preserving deterministic execution semantics.

## Implementation Checkpoint

- `POST /api/teams/:id/tasks/:task_id/messages` now allows omitted `from_actor_id`.
- Backend auto-fills sender as authenticated canonical user actor (`user:<id>`) when missing.
- Task conversation payload now guarantees `correlation_id` (reuse if provided, generate UUIDv7 when missing).
- Team page task chat no longer sends explicit `"from_actor_id": "user"` for task conversation messages.
- Team page now submits task chat as plain conversation input; mention parsing + mailbox routing/fan-out moved to backend.
- Router + API tests cover the no-explicit-sender path.
