# Agent Time Triggers Panel

## Summary

Added a read-only AgentTimeTriggers collapsible panel below the OutputHeader in the agent detail
view. Operators can now inspect scheduled time triggers that agents created via
`agent_time_trigger_set` (MCP/CLI) without switching to a debug tab or querying the API directly.

## Background

The backend shipped `agent_time_triggers` persistence and dispatch in March
(`docs/journal/2026-03-19-team-agent-time-triggers-and-profile-updates.md`), followed by internal
gRPC and CLI surface hardening. The feature journal explicitly deferred a frontend panel decision.
This slice closes that gap with a read-only inspection surface.

## Scope

- `web/src/api.ts` — `AgentTimeTriggerRecord` type and `listAgentTimeTriggers` API client
- `web/src/components/agent_time_triggers_panel.tsx` (new) — collapsible `<details>` panel
- `web/src/components/agents_route_shell.tsx` — thread `authToken` prop, render panel
- `web/src/components/agents_root_page.tsx` — pass `auth.token` to shell
- `web/src/agents_route_shell.test.tsx` — adapt for new `authToken` prop

This slice does not include create/cancel actions from the UI; those remain MCP/CLI-only for now.

## Key Decisions

- **Read-only first**: the panel lists triggers with status, countdown, and message preview but
  does not expose create or cancel actions. Agents remain in control of their own triggers.
- **Extensible by kind**: trigger items are labeled by their `kind` field
  (`triggerKindLabel(kind)`) so future trigger types (e.g. cron-like, event-driven) can be
  displayed without changing the panel structure.
- **Collapsible placement**: the panel sits between the OutputHeader and the workbench area,
  reusing the same `<details>` visual pattern as `OutputHeaderDetails`. It only appears when an
  agent is selected.
- **Active-count badge**: a blue pill badge on the summary shows how many triggers are in
  `scheduled` or `dispatching` state.

## Validation

```bash
cd web && pnpm exec tsc --noEmit       # clean
cd web && pnpm run lint                # clean
cd web && pnpm run build               # 637ms, 1213 modules
cd web && pnpm exec vitest run src/agents_route_shell.test.tsx  # 7/7 passed
```

PR: https://github.com/hawkingrei/agenthub/pull/519

## Follow-Ups

- Add inline create/cancel actions to the panel when operator-controlled triggers are needed.
- Surface triggers in the Team member console so operators see per-member scheduled work without
  navigating to individual agent pages.
- Extend `triggerKindLabel` when new trigger kinds land (cron, event-driven, etc.).
