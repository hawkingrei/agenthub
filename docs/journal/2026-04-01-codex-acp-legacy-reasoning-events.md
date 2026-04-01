## Summary

Restored ACP `agent_thought` emission for Codex sessions that still surface legacy reasoning
events instead of the newer `ReasoningContentDelta` shape.

## Why

Live Team ACP sessions could render tool calls and agent messages, but header-level `thinking Ns`
never appeared and the conversation view contained no `agent_thought` rows.

Direct inspection of the persisted ACP timeline for a live worker session on
`agenthub.hawkingrei.com` showed recent event types like:

- `agent_message`
- `tool_call`
- `tool_call_update`

with zero `agent_thought` rows.

`agenthub-codex-acp` already mapped the newer Codex reasoning events
(`ReasoningContentDelta`, `ReasoningRawContentDelta`, `AgentReasoning`) into
`SessionUpdate::AgentThoughtChunk`, but the live event handler still ignored the older compatibility
variants `AgentReasoningDelta`, `AgentReasoningRawContentDelta`, and
`AgentReasoningRawContent`.

## What Changed

- updated `agenthub-codex-acp/src/thread.rs` so live prompt handling emits ACP thought chunks for:
  - `AgentReasoningDelta`
  - `AgentReasoningRawContentDelta`
  - `AgentReasoningRawContent`
- tightened final reasoning deduplication so `AgentReasoning` and
  `AgentReasoningRawContent` cannot both emit duplicate `AgentThoughtChunk` updates for the same
  turn after deltas or after a prior final reasoning event
- updated Team ACP direct-control routing so detached ACP event loading and input sending require a
  resolved `agent_id` instead of falling back to `member_id`
- added focused regression tests covering:
  - legacy reasoning delta -> `AgentThoughtChunk`
  - legacy raw reasoning without deltas -> `AgentThoughtChunk`
  - dual final legacy reasoning events deduplicate to a single thought chunk
  - detached ACP event loading clears state when no runtime `agent_id` is available

## Validation

- `cargo test -p agenthub-codex-acp legacy_reasoning -- --nocapture`
- `cargo test -p agenthub-codex-acp raw_reasoning -- --nocapture`
- `cargo test -p agenthub-codex-acp final_reasoning -- --nocapture`
- `cd web && npx vitest run src/pages/team/use_team_actions.test.tsx`
- `cargo fmt --manifest-path agenthub-codex-acp/Cargo.toml --all`
- `git diff --check -- agenthub-codex-acp/src/thread.rs web/src/pages/team/use_team_actions.ts web/src/pages/team/use_team_actions.test.tsx web/src/pages/team_page.tsx`
