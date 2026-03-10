## Summary

PR #118 changed Team lifecycle so `create_team` auto-starts member runtime. Existing team API/router tests still used seeded worker agents configured with `worktree_mode=use_existing`, which now fails worker runtime policy during team creation and caused Rust/Bazel CI regressions.

## Changes

- Updated default team test fixtures so worker-like seeded member agents are configured with `create_worktree` and a repo-backed worktree root.
- Updated the router deletion regression to reuse the seeded `planner` agent instead of inserting a duplicate `agents.id`.
- Replaced the router "real executor" convergence test with the current member-owned runtime behavior:
  - stop team runtime first;
  - create run;
  - orchestrator marks the step `input_required` and tells the caller to start the team runtime instead of pretending the run owns session startup.

## Validation

- `cargo test teams_api_ -- --nocapture`
- `cargo test teams_router_ -- --nocapture`

## Notes

- This change intentionally aligns test fixtures with the new Team runtime model instead of adding hidden auto-creation or run-owned session fallback paths.
