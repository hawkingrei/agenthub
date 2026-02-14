# A2A Team Actor Runtime Context Start API

## Summary

Replace process-level env discovery for actor runtime skill injection with
explicit start-time context passed through the Agent start API.

## Background

Actor runtime skill injection previously depended on parent-process env vars
(`AGENTHUB_ACTOR_RUN_ID`, `AGENTHUB_ACTOR_ID`, etc.). That coupling made
scheduler-driven per-run/per-step execution brittle and hard to reason about in
multi-run scenarios.

## Scope

- `src/agent/manager.rs`
- `src/agent/manager/tests.rs`
- `src/api/agents.rs`
- `docs/todo.md`

## Key Decisions

1. Add explicit `AgentManager::start_agent_with_actor_context(...)` and make
   legacy `start_agent(...)` delegate to it with `None` context.
2. Remove env-based actor runtime context sourcing from `AgentManager` startup
   flow.
3. Extend `POST /api/agents/:id/start` to accept optional JSON body:
   - `actor_runtime.run_id` (required when `actor_runtime` is present)
   - `actor_runtime.actor_id` (required)
   - `actor_runtime.channel` (optional, defaults to `default`)
   - `actor_runtime.actor_cli_path` (optional, defaults to current executable
     path or `agenthub`)
4. Keep subprocess env export for actor CLI compatibility, but source values
   from explicit runtime context instead of inherited process env.
5. Keep backwards compatibility for existing callers by preserving no-body
   `/start` behavior.

## Validation

```bash
cargo test parse_start_actor_runtime_context -- --nocapture
cargo test default_actor_cli_path_returns_non_empty_value -- --nocapture
cargo test parse_worktree_mode_ -- --nocapture
```

## Follow-ups

- Wire orchestrator step execution to call `/api/agents/:id/start` with
  per-step actor runtime context and verify inbox/ack/send loop end-to-end.
