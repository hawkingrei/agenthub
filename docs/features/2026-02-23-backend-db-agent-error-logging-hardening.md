# Backend DB/Agent Error Logging Hardening

## Background

Backend error visibility in DB and Agent runtime paths had two observability gaps:

- some best-effort DB updates in agent lifecycle flows ignored failures (`let _ = ...`) without structured logs
- some DB failures bubbled up as generic API/internal errors without operation-level context

This made root-cause analysis slower during startup/exit edge cases and database maintenance failures.

## Scope

- Added structured, context-rich logging for non-fatal DB failures in Agent lifecycle paths:
  - startup failure recording and status fallback updates
  - stop/cancel session updates
  - send-input fallback updates when in-memory handle is absent
  - process-exit finalization updates and completion notification failures
- Improved Agent runtime event persistence logs with `agent_id/session_id/status/stream` context.
- Added DB module logging for:
  - sqlite open/connect failures
  - parent directory creation failures
  - cleanup delete failures in `cleanup_agent_event_history`

## Key Decisions

1. Keep behavior unchanged, improve observability first.

- Existing control flow and return semantics are preserved.
- Changes focus on replacing silent ignores with structured `tracing` logs.

2. Log both primary and compensating-path failures.

- Startup/cleanup often includes best-effort compensation (`record_failed_session`, status update).
- Compensating operations now log explicitly when they fail.

3. Prefer operation-level log messages.

- Log records include operation intent and stable identifiers (`agent_id`, `session_id`) to support production triage.

## Files

- `src/agent/manager.rs`
- `src/agent/manager/runtime.rs`
- `src/db.rs`

## Validation

Executed locally:

- `cargo test -p agenthub spawn_output_reader_promotes_latest_codex_event_types_for_acp_agents -- --nocapture`
- `cargo test -p agenthub send_input_does_not_mark_running_session_exited_while_agent_is_starting -- --nocapture`
- `cargo test -p agenthub init_db_creates_schema_and_enforces_foreign_keys -- --nocapture`
- `cargo test -p agenthub cleanup_agent_event_history_deletes_rows_older_than_retention -- --nocapture`
- `cargo test -p agenthub create_parent_dir_returns_error_when_parent_is_file -- --nocapture`
- `cargo test -p agenthub try_connect_returns_error_for_directory_path -- --nocapture`
- `cargo test -p agenthub init_db_at_path_returns_error_for_directory_path -- --nocapture`
- `cargo test -p agenthub cleanup_agent_event_history_returns_error_without_agent_events_table -- --nocapture`

All commands passed.
