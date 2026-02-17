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
   - use `GET /api/teams/:id/runs` with `limit`, `offset`, and `status`
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
cargo test --test web_assets styles_keep_acp_conversation_scoped
```

## Follow-ups

- Evaluate adding server-driven cursor pagination in API contracts for very
  large run volumes.
- Consider preserving per-team filter/offset state when switching between teams.
