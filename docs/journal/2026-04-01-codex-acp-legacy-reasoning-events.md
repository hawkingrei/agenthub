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
- kept the existing deduplication behavior so a later non-delta raw reasoning event does not
  duplicate already-streamed thought chunks
- added focused regression tests covering:
  - legacy reasoning delta -> `AgentThoughtChunk`
  - legacy raw reasoning without deltas -> `AgentThoughtChunk`

## Validation

- `cargo test -p agenthub-codex-acp legacy_reasoning -- --nocapture`
- `cargo test -p agenthub-codex-acp raw_reasoning -- --nocapture`
- `cargo fmt --manifest-path agenthub-codex-acp/Cargo.toml --all`
- `git diff --check -- agenthub-codex-acp/src/thread.rs`
