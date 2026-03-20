# Team Agent ACP Runtime Optimistic State

## Summary

`Teams -> Agents -> Agent ACP` could appear stopped immediately after `Start Team` even though shared-thread delivery already showed `Seen by N agents`.

The root cause was a split state path:

- shared-thread delivery/`Seen by` already used mailbox/runtime information;
- agent lifecycle badges and ACP session fallback still depended on `teamRuntimeByTeamId`;
- `startTeam` only applied an optimistic runtime update when a cached Team runtime record already existed.

If the page had no cached runtime record yet, member lifecycle fell back to stale catalog `AgentRecord.status` and the selected Team agent appeared stopped until the later runtime refresh arrived.

## Fix

- Extend `updateCachedTeamRuntimeStatus(...)` so it can synthesize a minimal optimistic `TeamRuntimeRecord` from Team member status data when no previous runtime cache exists.
- Pass current Team member statuses into `applyOptimisticTeamRuntime(...)` on both `Start Team` and `Stop Team`.
- Add regression coverage for the no-cached-runtime start case.

## Validation

- `cd web && npx vitest run src/pages/team/page_helpers.test.ts src/pages/team_page.smoke.test.tsx`
- `cd web && npm run lint -- src/pages/team/page_helpers.ts src/pages/team/page_helpers.test.ts src/pages/team_page.tsx src/pages/team_page.smoke.test.tsx`
- `cd web && npm run build`

## Follow-up

- Verify on deployed `agenthub.hawkingrei.com` that `Start Team` immediately flips Team member lifecycle/Agent ACP entry into a running state before the background runtime refresh completes.
