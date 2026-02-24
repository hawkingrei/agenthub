# Team Load Run Moved To Debug Run Ops

## Background

The Team run workbench primary controls were too dense. `Create Run` and `Load Run` were both exposed in the top run-control section, increasing visual pressure and competing with active-run operations.

## Scope

- `web/src/pages/team_run_panel.tsx`
- `web/src/pages/team_page.tsx`
- `web/src/pages/team_panels.test.tsx`

## Change Summary

1. Main run panel now focuses on primary flow:
   - Keep `Create Run`
   - Remove `Load Run` controls from the main `Run Controls` block
2. Add `Run Ops` under `Debug` tab:
   - Add debug tag `Run Ops`
   - Move `run_id` input + `Load Run` button there
   - Keep existing `Load Run` team-scope validation logic unchanged
3. Update Team panel tests to match the new interaction surface and text.

## Validation

- `npm --prefix web run lint`
- `npm --prefix web run test -- src/pages/team_panels.test.tsx src/pages/team_page.runs.test.ts`
