# Agents stale-session live switch

## Summary

- Problem: the Agents workbench could stay pinned to an older ACP `session_id` after the previous ACP server died and a new session started.
- Symptom: backend events and MCP/tool calls existed for the new session, but the UI still showed `No output yet` or an old conversation because live output was filtered by the stale `activeSessionId`.
- Scope: frontend-only session recovery in the Agents workbench.

## Evidence

- Live-domain inspection on `https://agenthub.hawkingrei.com/` showed the visible session label stayed on an older session while `/api/agents/:id/events?limit=200` returned events for a newer session.
- The stale session ended with `acp prompt error: Internal error: "server shut down unexpectedly"`.
- Frontend rendering already filtered live output by `activeSessionId`, so new-session MCP/tool-call events were cached but not displayed.

## Implementation

- Added `buildLatestLiveSessionMap(...)` to track the newest live `session_id` per agent from SSE batches.
- Added `resolveLiveSessionSwitch(...)` to detect when the currently selected agent receives live output on a different session.
- Updated the Agents workbench SSE consumer to:
  - refresh the per-agent latest-session map from incoming live events;
  - auto-switch `activeSessionId` to the new live session when the active agent is still running.

## Validation

- Focused web helper tests should cover:
  - latest live session tracking per agent;
  - switching to a newer live session for the active agent;
  - not switching when live output remains on the current session.

## Follow-up

- Consider a backend hint (`running_session_id` / `latest_session_id`) if we want a server-driven stale-session recovery contract instead of relying on frontend live-event inference alone.
