# Team Channel Send Feedback And Visibility

## What changed

- Team shared-channel sends now clear the draft immediately and insert a local optimistic echo before the HTTP round-trip completes.
- The send path now keeps a synchronous in-flight guard so repeated `Enter` presses or rapid `Send` clicks do not enqueue the same draft multiple times from one page session.
- Team channel composer now uses chat-style shortcuts: `Enter` sends, while `Shift/Ctrl/Cmd + Enter` stays as newline input.
- Team channel rendering now shows only user-visible conversation payloads (`chat_message`, `task_note`) plus explicit permission-review cards. Unknown ACP/debug payloads are no longer dumped into the visible channel stream as raw JSON.

## Validation

- `cd web && npm run test -- src/pages/team/use_team_conversation_actions.test.tsx src/pages/team_panels.test.tsx`
- `cd web && npm run lint -- src/pages/team/use_team_conversation_actions.ts src/pages/team_task_panel.tsx src/pages/team/use_team_conversation_actions.test.tsx src/pages/team_panels.test.tsx src/api.ts`
- `cd web && npm run build`

## Follow-up

- Shared-thread HTTP sends still rely on the page-local in-flight guard and optimistic echo for dedupe. The backend `POST /api/teams/:team_id/tasks/:task_id/messages` path does not yet expose an explicit `idempotency_key`, so cross-tab / retry-safe dedupe remains a follow-up.
