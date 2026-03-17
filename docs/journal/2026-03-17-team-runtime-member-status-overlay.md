# 2026-03-17 Team Runtime Member Status Overlay

## Summary

- Team member lifecycle rendering now prefers live Team runtime member status over the static agent
  catalog status.
- This keeps the Team header summary, member strip, and sidebar rows aligned after `Start Team`.

## Why

The live Team page exposed an inconsistent state on `agenthub.hawkingrei.com`:

- the Team header switched to `TEAM RUNNING` and `1/1 ONLINE`;
- the lower Team summary still said `1 OFFLINE`;
- the left-rail leader row still rendered `STOPPED`.

The root cause was that Team member lifecycle UI derived from `AgentRecord.status` and fallback
agent discovery only, while Team start/stop actually updates `TeamRuntimeRecord.members[]` first.
If the agent catalog still reports `stopped`, the Team surface stayed contradictory even though the
Team runtime itself was already running.

## What Changed

- `web/src/pages/team/member_helpers.ts`
  - `resolveTeamMemberAgentStatuses` now accepts optional Team runtime members.
  - Status resolution order is now:
    1. runtime `session_status`
    2. runtime `agent_status`
    3. catalog/fallback `AgentRecord.status`
  - Runtime member presence no longer counts as a missing agent.
- `web/src/pages/team_page.tsx`
  - Team member status maps now pass `teamRuntimeByTeamId[team.id]?.members` into the resolver so
    Team runtime state overlays stale catalog state immediately.
- `web/src/pages/team_page.runs.test.ts`
  - added coverage that runtime `running` status wins over stale catalog `stopped` state.

## Validation

- `cd web && npx vitest run src/pages/team_page.runs.test.ts`
- `cd web && npm run lint -- src/pages/team/member_helpers.ts src/pages/team_page.tsx src/pages/team_page.runs.test.ts`

## Chrome MCP

- Live baseline on `https://agenthub.hawkingrei.com/teams/<team_id>` showed a running Team header
  alongside stale member-level `STOPPED` / `OFFLINE` indicators.
- Post-edit live regression remains blocked until this change is deployed because verification must
  stay on the production domain.
