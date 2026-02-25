# Team Runs Tab And Tab Routing Refactor

## Background

Team workbench previously rendered `Run Browser` outside the top tab model.  
This made the product flow harder to read:

- conversation lane and execution lane were visually mixed;
- run-scoped tabs repeated run-context logic inline;
- no-active-run fallback behavior was not centralized.

## Scope

- `web/src/pages/team/state.ts`
- `web/src/pages/team/state.test.ts`
- `web/src/pages/team_page.tsx`
- `web/src/pages/team_tabs_bar.tsx` (new)
- `web/src/pages/team_active_run_panel.tsx` (new)
- `web/src/pages/team_panels.test.tsx`
- `docs/features/agents-teams.md`
- `docs/features/frontend-design.md`

## Key Decisions

1. Move run entry into tabs:
   - Add dedicated `Runs` tab for run browsing and `Start Team`.
   - Keep run-selection/start semantics in one place.

2. Keep conversation always available:
   - `Conversation` remains accessible without active run.
   - Human planning lane is not blocked by execution state.

3. Centralize tab policy in Team state:
   - Add `TEAM_TAB_ITEMS` metadata for tab values/labels.
   - Add shared `tabRequiresActiveRun(tab)` policy.

4. Extract shared composition components:
   - `TeamTabsBar` for top-level tab rendering.
   - `TeamActiveRunPanel` for shared active-run header/actions.

5. Unify no-active-run UX for run-scoped tabs:
   - One fallback card with `Go to Runs` action.
   - Avoid per-tab duplicated gating/fallback branches.

## Validation Evidence

- `npm --prefix web run test -- team/state.test team_panels team_page.runs team/use_team_run_effects team/use_team_mailbox_actions team/use_team_mailbox_effects`
- `npm --prefix web run build`
- MCP smoke checks:
  - baseline: `Run Browser` existed outside top tabs;
  - post-change: `Runs` tab is the run-entry lane and no-active-run fallback is centralized for run-scoped tabs.

## Notes

- This change is a UI/product-structure refactor only; backend run/step/mailbox contracts are unchanged.
- Debug-only internal controls remain under `Debug` and are not promoted into the primary human lane.
