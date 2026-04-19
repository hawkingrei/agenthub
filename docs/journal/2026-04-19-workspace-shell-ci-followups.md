## Summary

- stabilized Team workspace E2E helpers after splitting `team_page.e2e.ts`
- aligned Team shell follow-up comments around runtime badge chrome, mailbox labeling, and sidebar focus treatment
- added focused regression coverage for mailbox title resolution and forge-agent cleanup on team-spec conflict

## Validation

```bash
cd web && npm run lint
cd web && npm run test -- vite.config.test.ts src/pages/team/use_team_workspace_view_model.test.tsx src/pages/team/use_team_management_actions.test.tsx src/pages/team_panels.test.tsx src/pages/team/team_workspace_header.test.tsx
cd web && npm run test:coverage
make build-web
```

## Notes

- `isTeamDetailReady(...)` now accepts both `/teams/:id` and `/workspace/teams/:id` route shapes and also uses the selected team menu aria-label as a stable readiness signal.
- `onCreateForgeAgent(...)` now performs best-effort cleanup only for deterministic `409 team spec changed` failures; ambiguous network failures still keep the created agent to avoid deleting a potentially linked record after an unknown backend outcome.
