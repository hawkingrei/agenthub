## Summary

Hardened Team runtime recovery for dirty Codex resumes, surfaced member-specific start failures,
and added a Team ACP debug action to force a selected member onto a fresh session.

## Why

Two failure modes were still too expensive to recover from:

- a Team member could be pinned to a persisted Codex session whose rollout history contained a
  `CustomToolCall` without a matching output, which later panicked during Codex normalization /
  compaction
- `Start Team` and related runtime controls could collapse concrete member-runtime failures into a
  generic `internal server error`, leaving operators without a safe recovery action

## What Changed

- added `TeamRuntimeStartError::MemberRuntimeStart` so Team runtime start/control paths can return
  typed member-scoped failures
- mapped member-runtime startup failures to a conflict response instead of a generic opaque `500`
- sanitized Team runtime start/force-new-session conflict text so API clients receive stable
  member-scoped failure summaries while detailed startup causes stay in server logs
- tightened Team member-agent lookup classification so only true `RowNotFound` cases map to
  `MissingMemberAgent`; other lookup failures stay internal
- added `force_team_member_new_session(...)` plus `POST /api/teams/{id}/members/{member_id}/force_new_session`
- Team member force-new-session clears the selected member's persisted ACP session, restarts only
  that member runtime, and leaves the rest of the Team untouched
- Team ACP debug now exposes the recovery control as `Force New Session`
- hardened worker worktree git invocations by forcing `git -c core.fsmonitor=false ...` on worktree
  discovery / add paths
- repaired dirty Codex rollout history before `load_session(...)` resumes a thread:
  - synthesize aborted outputs for missing `CustomToolCallOutput`
  - synthesize aborted outputs for missing `FunctionCallOutput` / shell-call output
  - drop orphan output items without a matching call
  - repair compacted replacement history too
  - use `HashSet`-backed presence checks so history repair remains linear for long resumed sessions
- updated focused tests so:
  - Team API route coverage uses a stable leader-only restart path
  - worker restart coverage exercises the runtime helper directly and preserves the error chain
- updated test schema to include `agent_persistent_sessions`

## Validation

- `cargo test -p agenthub teams_api_force_new_session_restarts_only_selected_member -- --nocapture`
- `cargo test -p agenthub force_new_session_restarts_worker_runtime_with_new_session_id -- --nocapture`
- `cargo test -p agenthub map_runtime_start_error_maps_member_runtime_failures_to_conflict -- --nocapture`
- `cargo test -p agenthub member_agent_lookup_ -- --nocapture`
- `cargo test -p agenthub-codex-acp repair_initial_history -- --nocapture`
- `npm --prefix web run test -- --run src/pages/team_panels.test.tsx`
- `cargo clippy --locked -p agenthub --all-targets -- -D warnings`
- `cargo clippy --locked -p agenthub-codex-acp --all-targets -- -D warnings`
- `cargo fmt --all --check`
- `git -c core.fsmonitor=false diff --check`

## Chrome Notes

- Remote baseline attempt against `https://agenthub.hawkingrei.com/teams/276a2682-9ce7-4af5-aa6c-f12575d13c37`
  returned `502 Bad gateway`, so live-browser baseline on the deployed Team page was unavailable at
  edit time.
- Use local/browser follow-up validation after merge to confirm the debug panel renders `Force New Session`
  and the recovery flow behaves correctly against a real dirty Codex session.
