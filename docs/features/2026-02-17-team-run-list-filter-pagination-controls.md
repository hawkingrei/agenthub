# Team Run List Filter And Pagination Controls

## Summary

Improve Team Workbench run browsing in `/teams` by adding explicit status filter
controls and paged loading for run list retrieval.

## Background

The Team run list view previously loaded a large fixed batch and rendered all
runs without user-level filtering. This made it harder to focus on active runs
in larger teams and provided no explicit pagination interaction in UI.

## Scope

- `web/src/pages/team_page.tsx`
- `web/src/pages/team_page.runs.test.ts`
- `web/src/styles.css`
- `docs/todo.md`

## Key Decisions

1. Add `Runs` toolbar controls in Team Workbench:
   - status filter (`all`, `submitted`, `working`, `input_required`,
     `completed`, `failed`, `canceled`)
   - explicit `Refresh Runs` action
2. Switch run list fetching to paged API requests:
   - use `GET /api/teams/:id/runs` with `limit`, `before_created_at`, and `status`
   - page size defaults to `50`
   - add `Load More` button with `runsHasMore` state
3. Keep behavior safe for active run workflows:
   - active run refresh/cancel paths remain unchanged
   - on `replace` refresh, keep the current active run in local list even when it
     is outside the current page window, to avoid unintended run switching
   - paged list merging is deduplicated by `run.id` and sorted by `created_at`
4. Add unit tests for run paging helpers to lock behavior:
   - filter-to-API mapping
   - page merge dedupe and update precedence
   - active-run preservation on replace refresh

## Validation

```bash
npm --prefix web run test -- src/pages/team_page.runs.test.ts
npm --prefix web run build
/bin/zsh -lc 'set -euo pipefail; npm --prefix web run dev -- --host 127.0.0.1 --port 5173 --strictPort >/tmp/agenthub-vite.log 2>&1 & VITE_PID=$!; trap "kill $VITE_PID 2>/dev/null || true" EXIT; for i in {1..30}; do curl -sf http://127.0.0.1:5173 >/dev/null && break; sleep 1; done; PLAYWRIGHT_NO_WEBSERVER=1 npm --prefix web run e2e -- tests/e2e/team_page.e2e.ts'
```

## Follow-ups

- Evaluate adding server-driven cursor pagination in API contracts for very
  large run volumes.
