# Team Runtime Control Capability Gate

## Summary

Top-level Team runtime control routes now use the `runtime:operate` user capability instead of plain
authenticated-user authorization. Operators can stop and start their own Team runtime; viewers are
denied before runtime state changes are attempted.

## Background

The access-control rollout is migrating normal operator routes from coarse authentication-only gates
to product capability gates by route cluster. Team management mutations already use `teams:manage`;
this slice covers direct Team runtime lifecycle controls without changing Team read-only inspection,
run/task control, or upload routes.

## Scope

- Converted Team start and stop routes to `runtime:operate`.
- Converted Team member force-new-session route to `runtime:operate`.
- Added router coverage proving a `viewer` user is denied with `runtime:operate required`.
- Added router coverage proving an `operator` user can stop and start an owned Team runtime.

## Key Decisions

- Treat Team start, stop, and member session rotation as runtime operation because they change live
  runtime process state.
- Preserve existing resource boundaries after capability checks; Team ownership is still enforced by
  `load_team_for_user`.
- Leave Team run creation/cancel/resume/restart, run step controls, run mailbox actions, tasks, and
  uploads for separate route classification slices.

## Validation

```bash
cargo test -p agenthub api::teams::tests::teams_router_http_contract -- --nocapture
cargo test -p agenthub api::authz::tests::api_code_does_not_bypass_capability_authz_for_human_roles -- --nocapture
cargo fmt -p agenthub -- --check
git diff --check
```

## Follow-Ups

- Team run lifecycle and run step mutation routes are covered by
  [2026-07-21 Team Run Step Capability Gate](2026-07-21-team-run-step-capability-gate.md).
- Continue classifying Team read-only inspection, task, mailbox, and upload routes.
