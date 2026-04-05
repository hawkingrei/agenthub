# Team Conversation Selection Resilience

## What changed

- Team conversation selection now falls back to `selectedConversationDetail.task` when the task list is temporarily stale, so an already opened thread keeps its title and metadata even if the visible task list has not caught up yet.
- Team conversation refresh now reuses `selectedConversationLatestRun` when it is already known, instead of refetching `GET /api/teams/:team_id/tasks/:task_id` on every refresh cycle just to recover the latest run id.

## Validation

- `cd web && npm run test -- src/pages/team/page_helpers.test.ts src/pages/team/use_team_conversation_actions.test.tsx src/pages/team_page.smoke.test.tsx`
- `cd web && npm run lint`
- `cd web && npm run build`
- `make build-web`

## Notes

- This follow-up keeps the shared-thread `recent 20` window behavior unchanged.
- Live Team workbench MCP reload remained healthy after the change; the page stayed on `ONLINE · SSE CONNECTED`, with no new console regressions beyond the existing 404/network-change noise.
