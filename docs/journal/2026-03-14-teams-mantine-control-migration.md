# Teams Mantine Control Migration

## Summary

Migrated a narrow `/teams` control slice from custom controls to Mantine + Tailwind without changing Team workflow semantics.

Scope in this checkpoint:

- `TeamSidebar`
  - team filter input now uses Mantine `TextInput`
  - sidebar scope switch now uses Mantine `SegmentedControl`
- `TeamTasksPanel`
  - new-task title input now uses Mantine `TextInput`
  - task status filter now uses Mantine `SegmentedControl`
  - compile-preview context override now uses Mantine `TextInput`
- `TeamRunPanel`
  - run status filter now uses Mantine `NativeSelect`

The team actions overflow menu was intentionally left on the existing custom popup implementation for now. The Mantine `Menu` variant introduced more test/runtime instability than value in this slice, while the input/filter/select migration delivered the higher-yield framework convergence.

## Validation

### Frontend checks

- `cd web && npx vitest run src/pages/team_panels.test.tsx --pool=threads --maxWorkers=1`
- `cd web && npm run lint -- src/pages/team_sidebar.tsx src/pages/team_tasks_panel.tsx src/pages/team_run_panel.tsx src/pages/team_panels.test.tsx`
- `make build-web`

### Chrome DevTools MCP baseline/regression note

Baseline was taken on the deployed domain:

- URL requested: `https://agenthub.hawkingrei.com/teams`
- Actual page state: redirected unauthenticated shell at `/?next=%2Fteams`
- Visible content:
  - `AgentHub`
  - `Password + Passkey Login`
  - `Username`
  - `Password`
  - `Login`

This means the public-domain MCP check for this change only verified the unauthenticated baseline shell and redirect path. Authenticated `/teams` regression validation still needs a logged-in browser session on the deployed domain.

## Follow-up

- Verify the refreshed Mantine controls on an authenticated deployed `/teams` session.
- Continue the remaining `/teams` layout refresh work under the broader Slock-inspired, Mantine + Tailwind migration track.
