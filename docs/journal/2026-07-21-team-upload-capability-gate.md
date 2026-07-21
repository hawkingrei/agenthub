# Team Upload Capability Gate

## Summary

Team and Team-task upload routes now use the `teams:manage` user capability instead of plain
authenticated-user authorization. Operators can upload scoped Team artifacts for Teams they own;
viewers are denied before object upload records are created.

## Background

The access-control rollout is migrating normal operator routes from coarse authentication-only gates
to product capability gates by route cluster. Team definition and channel management already use
`teams:manage`; this slice applies the same management boundary to Team-scoped upload mutations.

## Scope

- Converted Team object and image upload helpers to `teams:manage`.
- Converted Team-task object and image upload helpers to `teams:manage`.
- Added router coverage proving a `viewer` user is denied with `teams:manage required`.
- Added router coverage proving an `operator` user can upload Team and Team-task scoped artifacts
  for an owned Team.

## Key Decisions

- Treat Team-scoped uploads as Team management because they create durable owner-scoped object
  records attached to Team resources.
- Preserve existing resource boundaries after capability checks; Team ownership, task lookup, and
  task/team matching still run before object upload creation.
- Leave Team read-only inspection, task mutations, and mailbox actions for separate route
  classification slices.

## Validation

```bash
cargo test -p agenthub api::teams::tests::teams_router_accepts_team_upload_route -- --nocapture
cargo test -p agenthub api::authz::tests::api_code_does_not_bypass_capability_authz_for_human_roles -- --nocapture
cargo fmt -p agenthub -- --check
git diff --check
```

## Follow-Ups

- Continue classifying Team read-only inspection routes.
- Team run mailbox operation routes are covered by
  [2026-07-21 Team Mailbox Capability Gate](2026-07-21-team-mailbox-capability-gate.md).
- Continue migrating Team task mutation routes to explicit capability gates.
