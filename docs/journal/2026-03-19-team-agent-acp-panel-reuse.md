# Team Agent ACP Panel Reuse

## Summary

- moved `Teams -> Agent ACP` from a bespoke `AcpConversation + InputDock` assembly onto the shared `AcpPanel` shell used by `/agents`
- kept the Team-specific session source and send path, but aligned the UI shell to `Conversation / Plan / Debug`
- preserved previously loaded same-session ACP history during periodic refresh so scrolling upward no longer causes older messages to disappear and reload repeatedly

## Why

The Team ACP surface had drifted from `/agents` in two ways:

1. refreshes replaced the latest ACP window and could drop already loaded older history
2. the panel chrome was custom, so tabs, jump behavior, plan/debug affordances, and future ACP improvements would continue to diverge

This change keeps Team member ACP on the Team data source while reusing the shared ACP shell and history semantics.

## Implementation

- `web/src/pages/team/page_helpers.ts`
  - `upsertAgentEventList(...)` now uses `mergeOutputsPreserveHistory(...)` for same-session replace refreshes
- `web/src/pages/team/use_team_actions.ts`
  - `loadMemberEvents("replace")` preserves already loaded older history for the same session instead of resetting `hasMore` and dropping older items
- `web/src/components/acp_panel_helpers.ts`
  - extracted shared input-dock jump resolution out of `app.tsx`
- `web/src/pages/team_member_acp_panel.tsx`
  - now renders through `AcpPanel`
  - derives Team-local conversation/debug props from member events
  - reuses the same `Conversation / Plan / Debug` shell and input-dock jump behavior as `/agents`
- `web/src/app.tsx`
  - switched to the shared `acp_panel_helpers` helper

## Validation

- `cd web && npx vitest run src/pages/team_panels.test.tsx src/pages/team/page_helpers.test.ts src/pages/team/use_team_actions.test.tsx src/app.input_dock_jump_mode.test.ts`
- `cd web && npm run lint -- src/pages/team_member_acp_panel.tsx src/pages/team_panels.test.tsx src/pages/team/page_helpers.ts src/pages/team/page_helpers.test.ts src/pages/team/use_team_actions.ts src/pages/team/use_team_actions.test.tsx src/app.tsx src/app.input_dock_jump_mode.test.ts src/components/acp_panel_helpers.ts`
- `cd web && npm run build`

## Follow-up

- still not fully unified with `/agents` data loading: Team ACP uses Team member event refresh effects rather than the standalone agent event/SSE pipeline
- deployed verification should confirm:
  - upward history scrolling remains stable while refresh/polling continues
  - `thinking` and tool-call rendering match `/agents`
  - `Conversation / Plan / Debug` shell appears identically under `Teams -> Agents -> Agent ACP`
