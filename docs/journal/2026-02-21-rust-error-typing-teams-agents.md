# Rust Error Typing For Team Resume And Agent Send Input

## Background

Two API paths used string matching on `anyhow::Error` text to map business errors:

- team run resume conflict (`completed run cannot be resumed`)
- agent send-input stale session conflict (`agent session mismatch`)

This was fragile and could silently break if error messages changed.

## Scope

- Introduce typed errors for the two business cases.
- Keep existing API behavior and response text stable.
- Update focused tests to assert typed errors where appropriate.

## Key Decisions

1. Add `TeamRunResumeError::CompletedRun` in `TeamManager` and return it from
   `resume_run` when status is `Completed`.
2. Map `resume` API conflict by downcasting to `TeamRunResumeError` instead of
   string `contains` matching.
3. Add `AgentSendInputError::SessionMismatch { expected, running }` in
   `AgentManager` and return it from `send_input` for stale session requests.
4. Map `send_input` API conflict by downcasting to `AgentSendInputError`.
5. Re-export typed errors from `team` and `agent` modules to keep API-layer
   mapping clear and explicit.

## Validation

Executed locally:

```bash
cargo test -q resume_run_handles_active_terminal_and_completed_statuses
cargo test -q send_input_rejects_stale_session_id_with_conflict
```

Both passed.

## Follow-up

- Continue replacing string-based error mapping in other API paths (`teams`,
  `db migration` guard branches, and selected `agents` conflict paths).
- Phase-2 PR should split large modules without changing behavior.
