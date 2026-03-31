# Agents API Explicit Validation

## Summary

- move the most obvious `create_agent` request validation to the API boundary in `src/api/agents.rs`
- reject blank `send_input` payloads and blank optional identifiers before they reach the agent manager
- stop silently coercing unknown `worktree_mode` values to `use_existing`

## Background

`src/api/agents.rs` already handled some request validation explicitly (`source`, actor runtime guards, selected workdir/worktree rules), but two paths still depended too much on downstream behavior:

- `create_agent` still relied on manager error strings for several invalid request shapes
- `send_input` accepted blank `input`, `message_id`, and `session_id` values at the HTTP edge and delegated all interpretation to the manager

That made the HTTP contract less stable than `src/api/teams.rs`, where malformed inputs are rejected at the route boundary with explicit messages.

## Implementation

- normalize and validate `create_agent` request fields in `src/api/agents.rs`
  - `name` and `command` are required after trimming
  - `worktree_mode` must be one of `use_existing`, `create_worktree`, or `reuse_worktree`
  - optional `worktree_repo` / `worktree_ref` values are trimmed and rejected if provided as blank strings
  - `agent_loop_enabled=true` now requires:
    - non-empty `agent_loop.prompt`
    - `agent_loop.idle_seconds` within `10..=86400`
- normalize and validate `send_input` request fields in `src/api/agents.rs`
  - `input` must not be blank after trimming
  - optional `message_id` / `session_id` values are trimmed and rejected if blank

## Validation

- `cargo test -p agenthub create_agent_route_rejects_blank_name -- --nocapture`
- `cargo test -p agenthub create_agent_route_rejects_blank_command -- --nocapture`
- `cargo test -p agenthub create_agent_route_rejects_invalid_worktree_mode -- --nocapture`
- `cargo test -p agenthub create_agent_route_validates_agent_loop_when_enabled -- --nocapture`
- `cargo test -p agenthub send_input_route_rejects_blank_input_and_identifiers -- --nocapture`
- `git diff --check`
