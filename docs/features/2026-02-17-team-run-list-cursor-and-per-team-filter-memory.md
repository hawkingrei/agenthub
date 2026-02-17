# Team Run List Cursor Pagination And Per-Team Filter Memory

## Summary

Harden Team Workbench run browsing by switching run paging to server cursor
semantics and preserving run filter state per team.

## Background

`GET /api/teams/:id/runs` already exposes cursor-style pagination with
`before_created_at`, while Team UI used an offset-like local state. This mismatch
can cause repeated pages and unstable `Load More` behavior. Team UI also reused one
global status filter across teams, causing context loss when switching teams.

## Scope

- `web/src/pages/team_page.tsx`
- `web/tests/e2e/team_page.e2e.ts`
- `docs/todo.md`

## Key Decisions

1. Replace offset-like paging calls with cursor paging:
   - request uses `before_created_at` from the last loaded run in current page
   - `Load More` now strictly follows backend pagination contract
2. Keep run browser state per team:
   - `statusFilter`
   - `beforeCreatedAt` cursor
   - `hasMore`
3. Preserve current active-run stability logic while paging and refreshing.
4. Keep API/backend contract unchanged; this is a UI state and request wiring fix.
5. Add Playwright coverage for:
   - `before_created_at` cursor usage on `Load More`
   - per-team run status filter memory across team switching

## Validation

```bash
npm --prefix web run test -- src/pages/team_page.runs.test.ts
npm --prefix web run build
HTTP_PROXY= HTTPS_PROXY= ALL_PROXY= NO_PROXY=127.0.0.1,localhost npm --prefix web run e2e -- tests/e2e/team_page.e2e.ts --project=chromium
cargo test --test web_assets styles_keep_acp_conversation_scoped
```
