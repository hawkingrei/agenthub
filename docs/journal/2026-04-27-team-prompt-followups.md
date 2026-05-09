# Team Prompt Follow-Ups

## Summary

- aligned the active Codex backlog to the post-`openai/codex@rust-v0.125.0` reality by removing stale TODO items that still assumed the temporary fork pin was live or that the managed-skill / PR160 verification work was unfinished
- tightened the default Team leader and worker prompts so they explicitly treat `task` as the primary ownership object, keep `run` / `step` as execution-debug artifacts, and use `attempt` for one active push that ends when the task moves to `waiting` or `in_review`

## What Changed

- `docs/todo.md`
  - replaced the stale fork-pin TODO pair with one follow-up that matches the current official `openai/codex@rust-v0.125.0` baseline
  - removed the already-completed managed-skill materialization and PR160 sync verification TODO items from the active ACP backlog
  - retargeted the remaining MCP parallel-tool-call TODO to the current `openai/codex@rust-v0.125.0` baseline and this journal entry instead of the older `0.121` upgrade note
- `docs/journal/2026-04-24-codex-custom-tool-output-hotfix.md`
  - appended a follow-up that records the shift from the temporary fork pin to the official upstream baseline and explains what remains open
- `crates/agenthub-team-prompts/prompts/default_team_leader_prompt.txt`
  - added explicit vocabulary guidance for `task`, `attempt`, `run`, and `step`
- `crates/agenthub-team-prompts/prompts/default_team_worker_prompt.txt`
  - added the same execution-vocabulary guidance from the worker side
- `crates/agenthub-team-prompts/src/lib.rs`
  - extended prompt-contract assertions so future prompt edits cannot silently drop the new `task` / `attempt` / `run` rules

## Why

- `docs/features/team-execution-vocabulary.md` already defines the canonical `task` / `attempt` / `run` / `step` boundary, but the default Team prompts did not state that boundary plainly enough.
- That gap makes it easier for leader/worker runs to drift back into “run means everything” language even though the canonical Team flow is task-first.
- The Codex TODO surface also drifted behind reality after PR 430 and PR 433 merged; keeping stale fork-pin language in the active backlog makes the next backend slice harder to choose correctly.

## Validation

- `cargo test -p agenthub-team-prompts`

## MCP Parallel Tool Calls Follow-Up

### Summary

- validated the current official `openai/codex` baseline used by `agenthub-codex-acp` for AgentHub-managed MCP passthrough servers
- kept `supports_parallel_tool_calls = false` for ACP-provided HTTP and stdio MCP server definitions
- kept ACP SSE MCP server definitions unsupported for Codex session config propagation

### Why

Codex supports a per-server `supports_parallel_tool_calls` flag, and upstream built-in MCP definitions can opt in when they know the server is safe for concurrent tool calls. AgentHub-managed passthrough MCP definitions arrive through ACP as generic HTTP or stdio server declarations, and ACP does not currently provide a per-server concurrency contract or idempotency guarantee. Enabling the flag by default would make server-specific ordering assumptions that AgentHub cannot prove.

### Validation

- `cargo test -p agenthub-codex-acp codex_mcp -- --nocapture`
- `cargo fmt --all --check`

### Follow-Ups

- Add a per-server MCP concurrency capability before enabling parallel tool calls for any AgentHub-managed MCP server by default.
- Revisit opt-in parallel tool calls only for servers with explicit idempotency and ordering guarantees.
