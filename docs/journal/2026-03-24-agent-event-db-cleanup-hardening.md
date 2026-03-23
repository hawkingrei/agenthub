## Summary

Hardened per-agent event DB cleanup so team deletion no longer relies on fixed sleeps or lossy path handling.

## Why

The previous flaky cleanup change still had three structural problems:

- file deletion failures could be swallowed while `remove_agent_db` reported success
- async cleanup used blocking filesystem calls
- the delete-team regression test relied on timing instead of the final cleanup condition

## What Changed

- kept `remove_agent_db` error propagation intact after bounded retries
- switched cleanup to `tokio::fs::try_exists` and `tokio::fs::remove_file`
- treated `NotFound` as successful cleanup instead of a retryable failure
- built SQLite sidecar paths with `OsString` so non-UTF8 paths stay lossless
- logged WAL checkpoint failures before deleting the event DB files
- added a router-level regression test that removes an agent DB and verifies the reopened history is empty
- updated the team delete regression test to poll for per-agent event DB removal, including WAL/SHM sidecars, instead of reopening the DB or sleeping
- stopped `finalize_process_exit(...)` from recreating a deleted event DB when `agent_sessions` has already been removed by higher-level cleanup
- released the process-handle write lock before awaiting idle-gc cleanup in `finalize_process_exit(...)`

## Validation

- `cargo test -p agenthub-db remove_agent_db_retries_cleanup_and_reopens_empty_history -- --nocapture`
- `cargo test -p agenthub teams_api_delete_team_cascades_related_run_data -- --nocapture`
- `cargo test -p agenthub finalize_process_exit_skips_event_persist_when_session_row_is_missing -- --nocapture`
- `cargo clippy --locked -p agenthub-db --all-targets -- -D warnings`
- `cargo clippy --locked -p agenthub --all-targets -- -D warnings`
- `cargo fmt --all --check`
- `git -c core.fsmonitor=false diff --check`
