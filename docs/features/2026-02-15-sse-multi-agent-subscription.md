# SSE Multi-Agent Subscription

## Background

The previous SSE implementation subscribed only to the currently selected agent
(`activeAgent`) through `/sse/agents/:id`.
When users switched between agents, the client closed and reopened the stream,
causing temporary `connecting/reconnecting` state flips in the header badge.

For multi-agent workflows, this behavior is noisy and does not represent the
actual platform connectivity state.

## Scope

- Add a multi-agent SSE endpoint: `/sse/agents?ids=a,b,c`.
- Keep existing `/sse/agents/:id` endpoint for backward compatibility.
- Update frontend SSE target selection to subscribe once to all currently
  stream-eligible agents (`running` or `idle`).
- Keep conversation rendering filtered by current active agent/session while
  still updating background caches and status for other agents.

## Key Decisions

- Backend accepts a comma-separated `ids` query and deduplicates/normalizes IDs.
- Backend ignores non-running IDs during multi-agent subscribe and streams from
  all currently available agent output channels.
- Stream fan-in is implemented via per-agent forwarder tasks into a single mpsc
  channel, with heartbeat unchanged.
- Frontend computes SSE targets from the current agent list and keeps one
  EventSource connection for the whole target set.
- Frontend uses refs for `activeAgent`/`activeSessionId` to avoid reconnect on
  UI selection changes and only appends visible lines for the active view.

## Validation

```bash
cargo test sse::tests
npm --prefix web run test -- sse_targets.test.ts connection_status.test.ts
npm --prefix web run build
```

- Expect:
  - switching active agent no longer forces SSE reconnect when target set is unchanged;
  - header badge reflects global stream state instead of selected-agent-only state;
  - active conversation still shows only active agent/session content;
  - background agent status changes are still reflected in the agent list.

## Follow-up

- Add an end-to-end web test that runs two active agents simultaneously and
  verifies no reconnect is triggered when switching active tabs.
