## Summary

Moved `agenthub actor ...` onto authority-side internal gRPC for team/task/mailbox/time-trigger and permission-review flows.

## Why

The actor CLI was still able to fall back to local sqlite-backed managers in several paths. That made runtime behavior depend on local filesystem writability and let agent shells bypass the intended authority control plane.

## What Changed

- Added internal gRPC control endpoints for:
  - team context lookup
  - team task list/create/update
  - time trigger create/list/cancel
- Updated actor CLI execution to use internal gRPC clients only for:
  - `team-members`
  - `team-tasks`
  - `team-task-create`
  - `team-task-update`
  - `inbox`
  - `ack`
  - `send`
  - `time-trigger-set`
  - `time-trigger-list`
  - `time-trigger-cancel`
  - `permission-review-respond`
- Moved immediate mailbox hint emission to the authority-side internal service so CLI send no longer needs a local `TeamManager`.
- Removed obsolete actor CLI fallback helpers and dead tests tied to the old sqlite-backed path.
- Kept actor CLI env fallbacks centered on injected actor runtime env:
  - `AGENTHUB_ACTOR_ID`
  - `AGENTHUB_ACTOR_TEAM_ID`
  - `AGENTHUB_ACTOR_CURRENT_RUN_ID`

## Validation

- `cargo check -p agenthub`
- `cargo test -p agenthub actor_cli::tests -- --nocapture`
- `cargo test -p agenthub internal::service::tests -- --nocapture`
- `cargo clippy --locked -p agenthub --all-targets -- -D warnings`
- `cargo fmt --all --check`
- `git -c core.fsmonitor=false diff --check`
