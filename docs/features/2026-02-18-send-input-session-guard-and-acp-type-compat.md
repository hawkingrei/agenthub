# Send Input Session Guard And ACP Type Compatibility

## Background

Two reliability issues were observed in active agent sessions:

1. A user could send input from a stale UI session view. The backend accepted input for the currently running session, but the UI filtered events by the old session id, which looked like message loss.
2. New codex ACP event types (for example `plan`, `available_commands`, `current_mode`, `run_status`) were not consistently classified as ACP stream events in the backend stderr reader path.

## Scope

- Add a session guard for `POST /api/agents/:id/input`.
- Add frontend auto-recovery when the backend reports a session mismatch.
- Update ACP classification compatibility for latest codex ACP typed JSON events.
- Keep non-ACP process stderr classification unchanged.

## Key Decisions

- API request payload for send-input now supports optional `session_id`.
- Backend `send_input` checks `expected session_id` against the running in-memory session id and returns `409 conflict` on mismatch.
- Frontend parses session-mismatch conflict text, updates active session to the running one, reloads events for that session, and retries send once with the same `message_id`.
- ACP message classification in `is_acp_message` now treats any JSON object with a non-empty string `type` as ACP.
- ACP auto-classification in output reader is gated by agent mode (`detect_acp_messages`), so only ACP agents can promote stderr lines into ACP stream.

## Implementation Notes

- Backend
  - `src/api/agents.rs`: extend `SendInputRequest` with `session_id`; map session mismatch to HTTP `409`.
  - `src/agent/manager.rs`: extend `send_input` signature with `expected_session_id`; reject mismatches.
  - `src/agent/manager/runtime.rs`: gate ACP auto-classification with `detect_acp_messages`.
  - `src/agent/manager/codec.rs`: broaden `is_acp_message` type detection.
  - `src/ws.rs`: accept optional `session_id` for websocket input path and pass through to manager.
- Frontend
  - `web/src/api.ts`: include optional `session_id` in send-input payload.
  - `web/src/app.tsx`: add send-input mismatch parser and one-shot retry flow after session switch.

## Validation

Local targeted checks:

- `cargo test is_acp_message_`
- `cargo test send_input_rejects_stale_session_id_with_conflict`
- `npm --prefix web test -- app.permission_scope.test.ts`

## Follow-up

- Real-browser validation is still required for full UX confirmation (see `docs/todo.md` entries linked to this feature note).
