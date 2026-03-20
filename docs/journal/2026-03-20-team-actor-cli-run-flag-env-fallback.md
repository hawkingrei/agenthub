## Summary

Fixed the `agenthub actor team-members --run-id ...` parsing contract so an explicit run id suppresses `AGENTHUB_ACTOR_TEAM_ID` env fallback. This keeps the CLI behavior symmetric with the existing `--team-id` path, which already suppresses env `run_id`.

## Why

Rust CI failed in `actor_cli::tests::parse_team_members_accepts_run_id_flag`. The old parser still populated `team_id` from env fallback even when `--run-id` was explicitly provided, and the test itself did not lock/restore env state. That made the command semantics surprising and allowed env leakage to surface in coverage runs.

## Changes

- `src/actor_cli.rs`
  - ignore env `team_id` fallback when `--run-id` is explicitly present
  - lock and restore env vars in `parse_team_members_accepts_run_id_flag`

## Validation

- `cargo test parse_team_members_accepts_run_id_flag -- --nocapture`
- `cargo test parse_team_members_accepts_team_id_flag_without_run -- --nocapture`
