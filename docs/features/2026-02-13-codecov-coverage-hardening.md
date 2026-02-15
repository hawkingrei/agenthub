# Codecov Coverage Hardening for Team + Actor Paths

## Background

Codecov reported patch coverage gaps around Team phase-1 backend changes, especially in:

- `src/db.rs`
- `src/team/manager.rs`
- `src/api/teams.rs`
- `src/state.rs`
- `src/api/mod.rs`

The actor mailbox state machine in `crates/agenthub-team-actor` also had limited direct unit coverage.

## Scope

This change focused on high-value branch coverage and review-driven risk points.

- Added unit tests for database bootstrap and SQLite foreign key enforcement.
- Hardened and tested Team run cancellation behavior for active vs terminal step states.
- Sanitized Team API internal error responses while preserving server-side logging.
- Added unit tests for AppState root bootstrap and safe-path seeding behavior.
- Added unit tests for API health endpoint.
- Added a full mailbox state-machine test set in `agenthub-team-actor` for send/ack/retry/dead-letter flows.

## Key Decisions

1. **Enable SQLite foreign key checks explicitly**

- `try_connect` now enables `foreign_keys(true)`.
- New tests assert both pragma state and real FK rejection behavior.

2. **Avoid stale step-cancel events during run cancel**

- `cancel_run` now updates steps with a terminal-status guard and emits `step_canceled` only when the update changed a row.

3. **Do not leak internal SQL errors from Team API**

- Team API internal failures now return a generic `internal server error` message to clients.
- The original error is still logged on the server for diagnosis.

4. **Make actor mailbox behavior executable and deterministic**

- Added deterministic relay tests for:
  - successful delivery
  - retry policy
  - dead-letter policy on retry exhaustion
  - permanent relay failure
  - argument normalization (`limit`, `max_attempts`, `retry_delay_secs`)

## Validation

Recommended commands:

```bash
cargo test -p agenthub-team-actor
cargo test db::tests -- --nocapture
cargo test state::tests -- --nocapture
cargo test cancel_run_only_cancels_active_steps -- --nocapture
cargo test teams_api_internal_errors_are_sanitized -- --nocapture
```

