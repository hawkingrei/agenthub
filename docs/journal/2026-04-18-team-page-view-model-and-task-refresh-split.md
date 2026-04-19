# Team Page View-Model And Task Refresh Split

## Summary

- extracted Team workspace title/lens/notice/view-model derivations from `web/src/pages/team_page.tsx` into `web/src/pages/team/use_team_workspace_view_model.ts`
- extracted Team task/shared-thread workspace selection and refresh coordination into `web/src/pages/team/use_team_task_workspace_data.ts`
- kept `team_page.tsx` focused on orchestration, panel wiring, and top-level effects

## Validation

```bash
make build-web
cd web && npm run test -- vite.config.test.ts src/pages/team_page.smoke.test.tsx src/pages/team_panels.test.tsx src/pages/team/team_page_header.test.tsx
```

## Notes

- `team_page.tsx` reduced from the previous post-shell-split shape to `3113` lines after moving the view-model and task refresh blocks out
- `use_team_workspace_view_model.ts` now owns workspace lens routing, workspace header copy, notice state, and mailbox display-name mapping
- `use_team_task_workspace_data.ts` now owns shared-thread/task refresh, stale selection cleanup, and selected conversation/task derivation
