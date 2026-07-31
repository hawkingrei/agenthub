# Team Channel Send Feedback And Visibility

## What changed

- Team shared-channel sends now clear the draft immediately and insert a local optimistic echo before the HTTP round-trip completes.
- The send path now keeps a synchronous in-flight guard so repeated `Enter` presses or rapid `Send` clicks do not enqueue the same draft multiple times from one page session.
- Team channel composer now uses chat-style shortcuts: `Enter` sends, while `Shift/Ctrl/Cmd + Enter` stays as newline input.
- Team channel rendering now shows only user-visible `chat_message` payloads plus explicit permission-review cards. `task_note` and unknown ACP/debug payloads are no longer dumped into the visible channel stream as raw JSON.
- Follow-up cleanup keeps `idempotency_key` trimming logic shared between the HTTP API and manager storage path, and the SQLite bootstrap now reuses one helper to create the task-message idempotency index in both fresh-init and migration paths.

## Supersession

Stable send-feedback, idempotency, and human-visible payload filtering rules from this note now live
in `docs/features/team-channels-threads.md#11-composer-send-and-visibility-contract`. This journal
remains the rollout evidence for the send feedback and visibility pass.

## Validation

- `cd web && npm run test -- src/pages/team/use_team_conversation_actions.test.tsx src/pages/team_panels.test.tsx`
- `cd web && npm run lint -- src/pages/team/use_team_conversation_actions.ts src/pages/team_task_panel.tsx src/pages/team/use_team_conversation_actions.test.tsx src/pages/team_panels.test.tsx src/api.ts`
- `cd web && npm run build`
- `CARGO_HOME=/tmp/agenthub-cargo-home CARGO_TARGET_DIR=/tmp/agenthub-cargo-target cargo test -p agenthub-db --locked --offline init_db_adds_task_message_idempotency_key_and_index_to_existing_messages_table -- --nocapture`
- `CARGO_HOME=/tmp/agenthub-cargo-home CARGO_TARGET_DIR=/tmp/agenthub-cargo-target cargo test -p agenthub --locked --offline map_task_message_error_ -- --nocapture`
- `CARGO_HOME=/tmp/agenthub-cargo-home CARGO_TARGET_DIR=/tmp/agenthub-cargo-target cargo test -p agenthub --locked --offline append_task_conversation_message_ -- --nocapture`
- `CARGO_HOME=/tmp/agenthub-cargo-home CARGO_TARGET_DIR=/tmp/agenthub-cargo-target cargo test -p agenthub --locked --offline team_task_messages_api_supports_idempotency_key_and_dedupes_mailbox_forwarding -- --nocapture`
- `CARGO_HOME=/tmp/agenthub-cargo-home CARGO_TARGET_DIR=/tmp/agenthub-cargo-target cargo test -p agenthub --locked --offline team_task_messages_api_ -- --nocapture`

## Follow-up

- The backend `POST /api/teams/:team_id/tasks/:task_id/messages` path now accepts `idempotency_key` and stores it under a per-conversation, per-sender uniqueness contract.
- Repeating the same request with the same `idempotency_key` now returns the original conversation message and preserves the same derived `correlation_id` when the client omitted one.
- Reusing an `idempotency_key` with a different normalized payload, route, or target now fails with HTTP `409 Conflict` instead of inserting a second message.
- Mailbox forwarding now stays deduped because only newly created task conversation messages are forwarded into `team_actor_messages`.
