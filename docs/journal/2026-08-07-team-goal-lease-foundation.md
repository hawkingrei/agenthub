# Team Goal Lease And Read-Only Fork Foundation

## Summary

Task execution claims now create a durable goal lease in the same SQLite transaction. The lease is
the Team and member concurrency reservation, rather than a separately maintained counter.
Bounded read-only forks are tied to the parent lease generation and return one immutable result to
the parent Task evidence stream.

## Background

Teamspace established one owner and one generation-fenced execution claim, but did not persist the
goal-level capacity needed to keep a member from silently starting overlapping work.

## Scope

- Add durable Task goal leases and active Team/member indexes.
- Reserve capacity with Task claim creation.
- Retain release history on explicit handoff and terminal Task completion or cancellation.
- Add Team-bounded fork creation, authorized list/create/complete APIs, and OpenAPI discovery.
- Fence completion on the active parent generation and atomically persist result evidence and audit.

## Key Decisions

- Active capacity is derived from unreleased, unexpired goal leases in the transaction that creates
  a claim.
- The default limits are three active goals per Team and one per member.
- Goal leases do not replace Task execution claims or change Step compatibility behavior.
- Task execution claims and goal leases advance one shared generation.
- Active forks are derived from incomplete, unexpired rows and capped at two per Team.
- The v1 fork result schema is the JSON object schema; result payloads are capped at 64 KiB.
- Fork results use the existing recursive sensitive-field redaction before durable persistence.
- Fork completion fails closed after parent release, expiry, handoff, or generation replacement.

## Validation

```bash
cargo test task_goal_capacity_is_reserved_per_member_and_released_at_terminal_status --lib
cargo test teamspace_invite_is_single_use_and_task_claim_is_single_owner --lib
cargo test goal_fork --lib
cargo test -p agenthub-db init_db_creates_schema_and_enforces_foreign_keys --lib
cargo test openapi_json_contains_team_runs_list_path --lib
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo fmt --all --check
git diff --check
bazel test --action_env=PROTOC=/opt/homebrew/bin/protoc \
  --test_arg=team::manager::tests::task_cases::goal_fork //:agenthub_unit_tests
bazel test --action_env=PROTOC=/opt/homebrew/bin/protoc \
  //crates/agenthub-db:agenthub_db_tests
```

On this macOS checkout, Bazel actions require the explicit `PROTOC` path above. The unfiltered
root unit-test target also expects a real `agenthub` binary in runfiles; that existing harness gap
is outside this slice, so the focused manager tests and database target are the Bazel evidence
boundary.

## Follow-Ups

- Add lease renewal, expiry recovery, and persisted conflict records.
- Enforce the read-only fork profile in each runtime adapter before dispatching fork work.
- Expose goal and fork state through the workbench UI and add deployed browser evidence.
