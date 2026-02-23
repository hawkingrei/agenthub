# Backend Error Chain And Agent Instrumentation

## Background

After the first backend logging hardening pass, runtime triage still had one visibility gap:

- many failures surfaced only as API boundary logs (`ApiError`), which looked like router-only errors
- DB/Agent root causes were present in error values but not consistently visible in structured logs

## Scope

- upgraded API error logging to include:
  - HTTP status
  - top-level error message
  - full `anyhow` error cause chain (`#0`, `#1`, ...)
- added `#[tracing::instrument(err)]` on AgentManager high-value runtime entrypoints so failures log from Agent module targets before API/router wrapping:
  - create/start/start-with-context/start-inner
  - prepare worktree
  - send input
  - stop/delete
  - ACP mode/model/config/cancel
  - code-mode toggle and startup exit reconciliation
- explicitly enabled `with_target(true)` in tracing formatter init for both stdout and file log sinks to keep module target visibility stable.

## Key Decisions

1. Keep behavior unchanged, improve observability boundaries.

- No API contract, DB schema, or lifecycle state-machine changes.
- Only logging visibility and attribution were improved.

2. Prefer module-local error attribution over router-only attribution.

- `instrument(err)` ensures errors are emitted at the failure domain (`agent::manager*`) even when they later bubble to API error mapping.

3. Preserve root cause details end-to-end.

- API layer now logs full `anyhow` cause chain, reducing "generic internal error" triage latency.

## Files

- `src/api/error.rs`
- `src/agent/manager.rs`
- `src/agent/manager/runtime.rs`
- `src/app.rs`

## Validation

Executed locally:

- `cargo test -p agenthub format_error_chain_keeps_root_causes -- --nocapture`
- `cargo test -p agenthub send_input_does_not_mark_running_session_exited_while_agent_is_starting -- --nocapture`
- `cargo test -p agenthub prepare_worktree_use_existing_mode_succeeds -- --nocapture`
- `cargo test -p agenthub set_code_mode_updates_agent_row -- --nocapture`
- `cargo test -p agenthub delete_agent_removes_related_rows -- --nocapture`
- `cargo test -p agenthub mark_exited_on_startup_returns_error_after_pool_close -- --nocapture`
- `cargo test -p agenthub finalize_process_exit_tolerates_closed_pool -- --nocapture`

All commands passed.
