# Summary

Allow `agenthub actor inbox` to fall back to the canonical Team shared-thread
mailbox run when actor runtime env exposes `team_id` but no `current_run_id`.

# Why

Shared-thread Team sessions can legitimately have mailbox traffic without a
member-scoped active run. In that state the old CLI stopped at parse time with
"run_id is required", even though the backend already keeps a canonical hidden
shared-thread mailbox run for the Team `all` thread.

# What Changed

- Relaxed `agenthub actor inbox` parsing so missing `current_run_id` no longer
  fails immediately.
- Added execute-time fallback:
  - if `--run-id` or `AGENTHUB_ACTOR_CURRENT_RUN_ID` exists, use it directly;
  - otherwise, if `AGENTHUB_ACTOR_TEAM_ID` exists, resolve Team scope through
    internal gRPC, locate the canonical shared-thread task, load its detail, and
    reuse the latest hidden shared-thread mailbox run id.
- Kept failure explicit when neither run scope nor Team scope is available.
- Updated help text to describe the Team shared-thread fallback.

# Validation

Suggested validation:

- `cargo test -p agenthub parse_inbox_allows_team_scope_without_current_run_id -- --nocapture`
- `cargo test -p agenthub resolve_shared_thread_task_id_prefers_canonical_shared_thread_task -- --nocapture`
- `cargo test -p agenthub grpc_team_task_client_handles_orphan_lists_and_detail_limit -- --nocapture`

# Follow-up

- This only fixes `actor inbox`.
- `ack` / `send` still need the broader issue `#244` run-scope inference work so
  all direct mailbox commands can reuse the same Team/runtime candidate
  resolution.

## Verified Evidence

- Focused validation covered parser fallback, canonical shared-thread task resolution, and gRPC
  client behavior around task detail and limit handling.
- `pull_request` CI for PR `#248`:
  - Bazel: `23710644844`
  - Rust: `23710644848`
  - Clippy: `23710644837`
  - Web: `23710644826`
  - Web E2E: `23710644842`
  - User Docs: `23710644841`
  - Distributed P2P Pipeline: `23710644840`
- default-branch `push` CI after merge commit `8af43bad`:
  - Bazel: `23710817588`
  - Rust: `23710817598`
  - Clippy: `23710817606`
  - Web: `23710817604`
  - Web E2E: `23710817594`
  - User Docs: `23710817603`
  - Distributed P2P Pipeline: `23710817599`
