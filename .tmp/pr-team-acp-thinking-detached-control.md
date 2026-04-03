## Summary

- fix Team member ACP event loading and input routing to target the resolved `agent_id` instead of
  `member_id`
- restore ACP `agent_thought` emission for Codex sessions that still emit legacy reasoning events
- keep the remaining mailbox / CLI formatting cleanup out of this PR

## Why

Two ACP regressions were stacked together in Team workspaces:

1. direct ACP control for a selected Team member could silently target the wrong `/api/agents/:id`
   route because Team UI state carried `member_id`, while ACP APIs require the backing `agent_id`
2. active `thinking` never appeared for some Codex-backed Team sessions because
   `agenthub-codex-acp` ignored legacy reasoning event variants that Codex still emits in some live
   sessions

This made a detached Team member session look idle or broken even when the session was still alive.

## Changes

- `web/src/pages/team_page.tsx`
  - resolve the selected Team member's backing `agent_id`
  - use that id for direct ACP send-input calls and Team ACP event loading
- `web/src/pages/team/use_team_actions.ts`
  - add `selectedMemberAgentId` and prefer it when loading detached ACP session events
- `web/src/pages/team/use_team_actions.test.tsx`
  - add regression coverage for the `agent_id` routing path
- `agenthub-codex-acp/src/thread.rs`
  - map legacy `AgentReasoningDelta`, `AgentReasoningRawContentDelta`, and
    `AgentReasoningRawContent` events into `SessionUpdate::AgentThoughtChunk`
  - keep existing reasoning-delta dedup semantics intact
- `docs/journal/2026-04-01-codex-acp-legacy-reasoning-events.md`
  - record the live-session root cause and validation trail

## Validation

- `cargo test -p agenthub-codex-acp legacy_reasoning -- --nocapture`
- `cargo test -p agenthub-codex-acp raw_reasoning -- --nocapture`
- `npx vitest run src/pages/team/use_team_actions.test.tsx src/pages/team_member_acp_panel.test.tsx src/pages/team_panels.test.tsx`
- `git diff --check -- agenthub-codex-acp/src/thread.rs web/src/pages/team/use_team_actions.ts web/src/pages/team/use_team_actions.test.tsx web/src/pages/team_page.tsx docs/journal/2026-04-01-codex-acp-legacy-reasoning-events.md`

## MCP

- baseline: `https://agenthub.hawkingrei.com/teams/276a2682-9ce7-4af5-aa6c-f12575d13c37`
  showed the selected worker ACP panel stuck at `session ... loading`, while persisted ACP history
  for the live session contained `agent_message` / `tool_call` rows but no `agent_thought`
- regression scope for this PR is adapter + routing logic; no new visual shell changes were made on
  top of the existing Team ACP UI
