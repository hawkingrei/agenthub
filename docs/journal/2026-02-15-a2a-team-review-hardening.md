# A2A Team Review Hardening

## Summary

Harden A2A Team orchestrator + agent start paths based on PR review findings:
restrict `actor_cli_path`, avoid actor-context session reuse, and treat
non-working dispatch transitions as failures with best-effort cleanup.

## Background

PR review identified a security-critical risk where user-provided
`actor_runtime.actor_cli_path` could be injected without restriction, plus two
correctness issues in orchestrator dispatch/state transitions.

## Scope

- `src/actor_runtime.rs`
- `src/api/agents.rs`
- `src/agent/manager.rs`
- `src/team/orchestrator.rs`
- `src/agent/manager/tests.rs`
- `docs/todo.md`

## Key Decisions

1. Centralize actor runtime helpers in `src/actor_runtime.rs`:
   - `default_actor_cli_path()`
   - `normalize_actor_cli_path(...)` with strict allow-list behavior (must
     resolve to the same canonical binary as server current executable)
   - `normalize_actor_context(...)`
2. Remove duplicated `default_actor_cli_path()` implementations from API,
   agent manager, and orchestrator modules.
3. Enforce CLI-path validation at API parse boundary and manager runtime
   boundary for defense in depth.
4. Change `start_agent_with_actor_context(...)` behavior:
   - keep existing session reuse only when no actor context is provided,
   - return conflict-style error when actor context is provided but agent is
     already running.
5. In `dispatch_step(...)`, treat non-`working` `start_step(...)` result as a
   failure and call `stop_member_agent(...)` best-effort to reduce leaked
   sessions.
6. Make orchestrator tick logging condition explicitly include
   `summary.dispatched > 0`.

## Validation

```bash
cargo fmt
cargo test actor_runtime::tests::
cargo test parse_start_actor_runtime_context
cargo test team::orchestrator::tests::dispatch_once
cargo test dispatch_step_returns_error_and_stops_member_when_step_is_not_working
```

## Follow-ups

- Add a configurable actor CLI allow-list policy (config-file based) for
  environments that require custom actor wrappers beyond the default binary.
- Add an integration test that simulates concurrent manual step mutation during
  orchestrator dispatch and verifies no orphan running sessions remain.
