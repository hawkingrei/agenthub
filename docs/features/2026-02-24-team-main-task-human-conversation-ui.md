# Team Conversation-First Human Planning UI

## Background

Team run orchestration already supports conversation APIs backed by internal `main_task` IDs:

- `POST/GET /api/teams/:id/main_tasks`
- `POST/GET /api/teams/:id/main_tasks/:main_task_id/messages`
- `POST /api/teams/:id/main_tasks/:main_task_id/compile_run_preview`

However, `/teams` UI still exposed `main_task` terminology and relied on manual ID input for
compile preview, instead of a first-class conversation flow.

## Scope

- Add web API bindings for Team conversation records/messages in `web/src/api.ts`.
- Add a dedicated conversation panel:
  - create conversation (title/topic/conversation mode)
  - select existing conversation
  - send planning messages with route modes (`to_leader`, `to_member`, `group_chat`)
  - view message timeline
  - refresh conversation list and message list
- Promote conversation panel to a primary tab (`Conversation`) ahead of run internals (`Overview`, `Events`, ...).
- Keep compile preview workflow but hide internal `main_task` naming from user-facing UI copy:
  - remove manual ID input; compile always targets currently selected conversation.
  - selected conversation changes automatically update compile target.

## Key Decisions

1. Keep run mailbox and planning conversation separated.

- Run mailbox remains member-only (`spec.members[].member_id`) for runtime agent coordination.
- Human planning interaction is surfaced via conversation APIs (internally backed by `main_task` IDs).

2. Keep sender identity explicit and backend-canonicalized.

- UI sends `from_actor_id = "user"` for human-origin conversation messages.
- Backend canonicalizes to `user:<authenticated_user_id>` and enforces route contracts.

3. Keep UI changes additive and low-risk.

- Conversation panel is promoted to primary workflow while Debug `Run Ops` remains for internal operations.
- Compile flow remains executable without exposing internal IDs, because selection state holds backend IDs.
- Primary run browser keeps `Start Team` as a visible quick action for manual team bootstrap/restart.
- Existing run/steps/mailbox controls are unchanged.

## Files

- `web/src/api.ts`
- `web/src/pages/team_main_task_panel.tsx`
- `web/src/pages/team_page.tsx`
- `web/src/pages/team_panels.test.tsx`
- `web/src/pages/team_run_panel.tsx`
- `web/tests/e2e/team_page.e2e.ts`
- `docs/todo.md`

## Validation

Local checks executed:

```bash
pnpm -C web run lint
pnpm -C web exec vitest run src/pages/team_panels.test.tsx src/pages/team_page.runs.test.ts
pnpm -C web run build
```

Added E2E scenario in `web/tests/e2e/team_page.e2e.ts`:

- `team conversation-first integration supports virtual team tiny-tool delivery flow`
