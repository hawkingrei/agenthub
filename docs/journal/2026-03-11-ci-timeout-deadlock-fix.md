# CI Timeout Deadlock Fix

## Summary

This change fixes the Bazel timeout regression affecting Team runtime tests.

## Root cause

Two issues combined to make Team runtime tests hang under CI:

1. `spawn_exit_watcher` held the child-process mutex while awaiting `child.wait()`.  
   `stop_agent()` then tried to lock the same mutex in order to kill the child, which created a deadlock during Team runtime shutdown and `delete_team`.

2. Team API test fixtures reused shared temp workdirs and worker worktree roots under `/tmp`, which made Bazel sandbox runs interfere with each other.

## Fixes

- Update `spawn_exit_watcher` to poll `try_wait()` without holding the child mutex across await points.
- Bound `stop_agent()` child wait with a timeout so cleanup cannot block forever.
- Add an ACP session startup timeout so hung actor runtime bootstrap fails fast instead of waiting for the outer CI timeout.
- Make Team API test fixture workdirs and worker worktree roots unique per test state.
- Ensure `team_runs_api_lists_team_runs_with_status_filter_and_cursor` cleans up created teams.

## Validation

- `cargo test teams_api_create_team_auto_starts_member_runtime -- --nocapture`
- Full CI validation is expected on PR checks after push.
