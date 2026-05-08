# Release P0 Validation

## Summary

- Verified the three release-blocking P0 frontend items after the latest `main` sync.
- Confirmed Team retention caps through focused unit coverage and deployed request shape.
- Confirmed the deployed Team workspace joins channel, Kanban, member ACP, runtime, and status surfaces under one workflow.
- Confirmed status surfaces now use SSE-first refresh with polling retained as bounded fallback behavior.

## Background

The P0+ implementation work has already landed. Before a release, the remaining
blocking checks were validation tasks rather than new feature work:

- Team web retention caps after merge.
- Deployed Team collaboration workflow on `agenthub.hawkingrei.com`.
- Remaining status surfaces that previously depended on primary polling.

## Scope

This pass only records post-merge validation evidence. It does not change runtime,
API, database, or frontend behavior.

## Key Decisions

- Treat the deployed `tidb fuzz/bugfix team` workspace as the release validation
  target because it exercises the normal long-lived Team workflow with real
  channels, Kanban tasks, and member ACP histories.
- Treat a fresh Chrome DevTools MCP reload as the source of truth for live request
  shape. Preserved request logs can include earlier tab switches and member
  changes, so they are useful for context but not for current polling conclusions.
- Keep member ACP events allowed to use bounded fallback refresh when the SSE
  stream has no non-heartbeat activity. This is distinct from app-level agent
  status polling, which stays disabled while the app-level agent SSE stream is
  connected.

## Validation

Local focused tests:

```bash
cd web && npm run test -- src/pages/team/page_helpers.test.ts src/pages/team/use_team_conversation_actions.test.tsx src/pages/team/use_team_runtime_effects.test.tsx src/pages/team/use_team_member_acp_effects.test.tsx src/use_app_permissions.test.tsx src/use_app_agents.test.tsx
```

Result:

- 6 test files passed.
- 102 tests passed.

Chrome DevTools MCP deployed check:

- URL:
  `https://agenthub.hawkingrei.com/workspace/teams/276a2682-9ce7-4af5-aa6c-f12575d13c37?lens=members&member=c319f933-1358-4418-a111-872304052422&tab=thread`
- Visible Team workspace state:
  - sidebar shows `# all`, `Kanban`, and three Team members;
  - member ACP thread renders active-thread output, update counts, tool-call groups,
    composer, and lifecycle state;
  - Kanban lens renders all task lanes and 100 tasks;
  - channel lens renders `# all` messages, thread buttons, delivery receipts, and
    the shared channel composer.
- Fresh reload request shape:
  - shared-thread channel messages load with `messages?limit=20`;
  - runtime status opens `/sse/teams/{team_id}/runtime`;
  - shared-thread messages open `/sse/teams/{team_id}/tasks/{task_id}/messages`;
  - member ACP opens `/sse/agents?ids={agent_id}`;
  - app-level `/api/agents` appears at initial load and does not continue as the
    primary periodic status path while SSE is connected.
- After a 15-second observation window:
  - runtime emitted one follow-up `/api/teams/{team_id}/runtime` refresh from SSE
    invalidation/fallback reconciliation;
  - member ACP events used bounded `/api/agents/{agent_id}/events?limit=60`
    fallback refresh while the member SSE stream stayed connected;
  - no unbounded `/api/agents` status polling loop was observed.
- Console:
  - no application runtime errors were observed;
  - a single unrelated `favicon.ico` 404 was observed.

## Follow-Ups

- The remaining open frontend P0 items are broader structural/governance checks,
  not blockers for these three release validation items.
- A future non-blocking cleanup can add a real `/favicon.ico` route or asset to
  remove the console noise.
