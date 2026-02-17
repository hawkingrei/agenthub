# Team Output Preview Limit Before Member Selection

## Summary

Update Team Workbench output behavior so unscoped Team views stay compact: before a
specific member is selected, only the latest 5 run records are shown.

## Background

`/teams` can surface a large amount of run output when a run is active. In Team mode,
users first need a concise global view, then can drill down into one member for full
history.

## Scope

- `web/src/pages/team_page.tsx`
- `web/src/pages/team_page.runs.test.ts`
- `docs/todo.md`

## Key Decisions

1. Add a run-record preview helper that limits output to latest 5 records when
   no member is selected.
2. Apply the preview policy in:
   - `Events` tab (run events list)
   - `Member Console` default view (before member selection)
3. Keep full history behavior for selected member unchanged.
4. Disable member-history paging (`Load Older`) unless a concrete member is selected.

## Validation

```bash
npm --prefix web run test -- src/pages/team_page.runs.test.ts
npm --prefix web run build
```

