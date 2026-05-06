# Team Authority Group ID

## Summary

The first physical `group_id` rollout slice landed on Team authority rows.
`team_definitions`, `team_tasks`, and `team_runs` now have nullable `group_id` columns, and new
Team-owned task/run authority rows inherit the Team group boundary.

## Background

The logical message metadata contract treats `group_id` as the future multi-tenant and distributed
routing boundary. Before message projections can rely on it, the control-plane authority rows need a
stable nullable storage location that is owned by `main`.

## Scope

- Added nullable `group_id` storage to Team definition, task, and run authority tables.
- Backfilled legacy Team rows from the existing `owner_user_id` compatibility boundary.
- Backfilled task and run rows from their owning Team definition.
- Propagated the Team `group_id` into newly created Team tasks, channels, shared-thread bootstrap
  tasks, shared-thread mailbox runs, and ordinary Team runs.

## Key Decisions

- This slice does not expose `group_id` as a public API contract yet.
- `owner_user_id` is used only as the current single-user compatibility boundary. It is not renamed
  or treated as the final long-term group model.
- Routing enforcement remains deferred until node registry rows and message authority/projection
  rows all carry compatible `group_id` values.

## Validation

```bash
cargo fmt --all --check
cargo test -p agenthub-db init_db_adds_and_backfills_team_authority_group_ids -- --nocapture
cargo check -p agenthub
cargo test -p agenthub create_team_task_and_run_persist_authority_group_id -- --nocapture
```

## Follow-Ups

- Add nullable `group_id` to node registry authority rows and keep node-local mirrors read-only.
- Propagate `group_id` into message authority rows and channel replica/search projections.
- Enforce cross-group routing only after both node and message authority rows are populated.
