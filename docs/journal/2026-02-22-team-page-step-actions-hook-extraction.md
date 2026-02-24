# Team Page Step Actions Hook Extraction

## Background

After extracting run/mailbox actions into `useTeamActions`, `TeamPage` still contained step execution callbacks (`onSubmitStep`, `onApplyStepAction`).
Those callbacks mixed validation, API calls, and post-action refresh orchestration directly in the page component.

## Scope

This change extracts step callbacks into a dedicated hook:

- Added `web/src/pages/team/use_team_step_actions.ts`
- Moved step callbacks from `TeamPage`:
  - `onSubmitStep`
  - `onApplyStepAction`
- Updated `web/src/pages/team_page.tsx` to consume the new hook
- Added focused tests in `web/src/pages/team/use_team_step_actions.test.tsx`

## Key Decisions

1. Keep step API orchestration in one token-bound hook.

- `useTeamStepActions` builds a token-scoped client with `useMemo`.
- `TeamPage` keeps only state wiring and callback usage.

2. Keep step refresh contract unchanged.

- After step submit or apply, the hook still refreshes run/steps/events/snapshot together.

3. Fix hidden dependency hazard while extracting.

- `depends_on` parsing is now local to the hook (`parseCsvList`) and explicitly tested.

## Validation

Executed locally:

- `npm run test -- use_team_step_actions`
- `npm run test -- src/pages/team`
- `npm run lint`
- `npm run build`

All commands passed.

## Follow-up

- Continue with the next split pass for debug/mailbox raw actions to further slim `TeamPage`.
