# Team Top Member Status Strip

## Background

Recent Team workbench changes made run controls and active-run tabs more explicit, but this also reduced immediate visibility of member runtime health.
When users open a Team and no active run is selected, they still need a quick answer to:

- which members are currently working;
- which are idle;
- which are stopped or missing.

## Scope

- Frontend only (`/teams`).
- No backend API changes, no DB migration.
- Reuse existing Team member status resolution (`spec.members` + agent records) instead of introducing new status endpoints.

## Key Decisions

1. Add a dedicated status strip at the top of selected Team main content.
2. Keep status labels user-facing and task-oriented:
   - `running`/`working` -> `working`
   - `idle` -> `idle`
   - `stopped`/`completed`/`failed`/`exited` -> `stopped`
   - unresolved member-agent mapping -> `missing`
3. Show summary counters (`working`, `idle`, `stopped`, `missing`) and per-member cards together.
4. Keep the strip visible regardless of active-run selection so users can diagnose member lifecycle before entering run-level tabs.

## Implementation

- Added `web/src/pages/team_member_status_strip.tsx`:
  - `normalizeTeamMemberLifecycle` maps raw agent status into display lifecycle buckets.
  - Top summary chips and per-member cards with `StatusBadge` tone mapping.
- Integrated strip into `web/src/pages/team_page.tsx`:
  - Compute `selectedTeamMemberStatuses` from existing `teamMemberStatusByTeamId`.
  - Render `TeamMemberStatusStrip` before `TeamRunPanel` for selected team.
- Added tests in `web/src/pages/team_member_status_strip.test.tsx`:
  - lifecycle normalization coverage;
  - summary and member-row rendering coverage;
  - empty-state coverage.

## Validation

Executed local checks:

- `npm --prefix web test -- src/pages/team_member_status_strip.test.tsx src/pages/team_page.runs.test.ts`
- `npm --prefix web run lint -- src/pages/team_member_status_strip.tsx src/pages/team_member_status_strip.test.tsx src/pages/team_page.tsx`
- `npm --prefix web run build`

All passed locally.

Chrome DevTools MCP verification:

- Baseline and post-change checks were attempted for `https://agenthub.hawkingrei.com/teams`.
- Both attempts were blocked by an occupied local `chrome-profile` lock in this environment, so no MCP snapshot was captured in this change.

## Follow-ups

- Verify the top status strip behavior on production/staging with Chrome DevTools MCP once profile-lock blocker is cleared.
- Record push and PR CI run IDs in PR evidence before marking TODO done.
