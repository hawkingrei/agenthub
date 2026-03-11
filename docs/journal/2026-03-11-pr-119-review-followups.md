# PR 119 Review Follow-ups

## Summary

Addressed the remaining PR #119 review comments with eight focused fixes:

1. Narrowed the `/teams` runtime status helper input type without rejecting callers that include `member_id`.
2. Updated the Rust team API test fixture to resolve the real `agenthub` binary path instead of depending on the test harness executable path.
3. Made the Playwright `/teams` runtime mock stateful so `/api/teams/:id/runtime` reflects `start` and `stop` transitions consistently.
4. Prevented `agenthub actor team-members --team-id ...` from silently inheriting a runtime `run_id` from the environment.
5. Cleared stale `session_id` values during `/teams` stop-runtime optimistic cache updates so the runtime badge no longer reports stopped members as online.
6. Moved test-only actor-context helpers out of `src/api/teams.rs` and into `src/api/teams/tests.rs` so the production router module no longer carries dead test helpers.
7. Removed `idle_gc` state eagerly during `stop_agent()` so explicit stops no longer rely on the exit watcher to clean idle-GC bookkeeping.
8. Replaced the free-form Team runtime `status` string with a dedicated `TeamRuntimeStatus` enum for backend/runtime read models and control responses.

## Validation

- `cargo test resolve_test_agenthub_binary_path_prefers_real_binary -- --nocapture`
- `cargo test -p agenthub parse_team_members_accepts_team_id_flag_without_run -- --nocapture`
- `cargo test -p agenthub team_member_actor_context_match_rejects_mismatched_team_runtime -- --nocapture`
- `cargo test -p agenthub stop_agent_removes_idle_gc_state_even_when_exit_watcher_exits_early -- --nocapture`
- `cargo test -p agenthub describe_team_runtime_returns_member_runtime_status -- --nocapture`
- `cd web && npx vitest run src/pages/team/page_helpers.test.ts --pool=threads --maxWorkers=1`
- `cd web && npm run lint`

## Notes

- The new Playwright runtime-control coverage was added to `web/tests/e2e/team_page.e2e.ts`.
- Local Playwright execution remains blocked in the current macOS sandbox because Chromium cannot complete Mach port bootstrap; the mock logic was still linted and kept in-tree for CI coverage.
