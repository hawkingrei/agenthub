# Team Page Management Hook Split

## Summary

- extracted Team create/forge/edit flows plus Team runtime and selected-agent management actions from `web/src/pages/team_page.tsx`
- introduced `web/src/pages/team/use_team_management_actions.ts` so the page keeps orchestration state while action handlers move behind a dedicated hook boundary
- kept existing Team shell behavior and render output intact while reducing inline callback density inside `TeamPage`

## Validation

```bash
make build-web
cd web && npm run test -- vite.config.test.ts src/pages/team_page.smoke.test.tsx src/pages/team_panels.test.tsx src/pages/team/team_page_header.test.tsx
```

## Follow-up

- `web/src/pages/team_page.tsx` is still well above the preferred file-boundary target, so the next extractions should focus on workspace view-model derivation and conversation/task refresh coordination.
