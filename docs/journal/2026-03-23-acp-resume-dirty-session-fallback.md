# ACP Dirty Resume Fallback

## Summary

- add a startup grace check for resumed ACP sessions
- if a resumed ACP session exits unsuccessfully during the grace window, clear the persisted ACP session id and retry once with `new_session`
- harden process finalization so an old exit watcher only cleans up the matching session handle

## Why

Debug builds can panic when Codex resumes a dirty session whose history contains a `CustomToolCall` without the matching output. In that case `load_session` appears to succeed, but the child exits immediately after startup. Previously AgentHub kept the dirty persistent session id and the next start would hit the same broken resume path again.

## Validation

- `cargo test -p agenthub agent::manager::session::tests -- --nocapture`
- `cargo test -p agenthub agent::manager::process::tests::finalize_process_exit_keeps_newer_running_handle_for_different_session -- --nocapture`
- `cargo clippy --locked -p agenthub --all-targets -- -D warnings`
- `cargo fmt --all --check`
- `git -c core.fsmonitor=false diff --check`
