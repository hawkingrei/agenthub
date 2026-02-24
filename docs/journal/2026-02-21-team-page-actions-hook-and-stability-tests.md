# Team Page Actions Hook Extraction And Stability Guard

## Background

`web/src/pages/team_page.tsx` still contained a large block of Team run/mailbox API callbacks (`refreshRun`, `refreshEvents`, `onCreateRun`, etc.).
The callback-heavy section increased review surface and made `useEffect` dependency reasoning harder.

## Scope

This change extracts Team run/mailbox action callbacks into a dedicated hook:

- Added `web/src/pages/team/use_team_actions.ts`.
- Migrated the following callback groups from `TeamPage` into the new hook:
  - run refresh/load actions (`refreshRun`, `refreshTeamRuns`, `refreshSteps`, `refreshEvents`, `refreshSnapshot`)
  - run operation actions (`onCreateRun`, `onLoadRunById`, `onRefreshRuns`, `onLoadMoreRuns`, `onCancelRun`, `onResumeRun`, `onRestartRun`)
  - mailbox/member fetch actions (`loadInbox`, `loadMemberEvents`)
  - bootstrap helpers (`refreshAgents`, `refreshTeams`)
- Updated `web/src/pages/team_page.tsx` to consume the hook and remove the duplicated inline callback block.
- Added focused stability tests in `web/src/pages/team/use_team_actions.test.tsx`.

## Key Decisions

1. Keep `TeamPage` as orchestration-only for these actions.

- The page now wires state + refs into `useTeamActions` and consumes returned callbacks.
- This trims `TeamPage` and isolates API-flow logic in one module.

2. Centralize token-bound API access in one memoized client.

- `useTeamActions` builds a token-scoped API client with `useMemo`.
- Callback identity now changes only when relevant dependencies (for example token or state inputs) change.

3. Preserve behavior while tightening step selection handling.

- `refreshSteps` now computes the next selected step id explicitly from current `selectedStepId` + fetched list.
- This avoids function-updater style usage for a string-only setter.

## Validation

Executed locally:

- `npm run lint`
- `npm run test -- use_team_actions`
- `npm run test -- src/pages/team`
- `npm run build`

All commands passed.

## Follow-up

- `TeamPage` remains large; step-action and debug-action blocks can be split into follow-up hooks (`useTeamStepActions`, `useTeamDebugActions`) in separate PRs.
