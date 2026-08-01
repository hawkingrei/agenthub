# Team Thread Reply Capability Gate

## Summary

Team channel thread reply writes now use the `runtime:operate` user capability instead of plain
authenticated-user authorization. Viewers are denied before thread reply state changes are
attempted.

## Background

The access-control rollout is migrating normal operator routes from coarse authentication-only gates
to product capability gates by route cluster. Team task mutation routes are already capability
gated; channel thread replies are also mutation paths because they write task-backed conversation
messages and can forward notifications to runtime mailboxes.

## Scope

- Converted the Team channel thread reply route to `runtime:operate`.
- Added router coverage proving a viewer is denied with `runtime:operate required`.
- Added router coverage preserving the allowed thread reply contract for an authorized operator
  path.
- Corrected the Team task capability journal follow-up so thread replies are no longer grouped with
  read-only inspection routes.

## Key Decisions

- Treat thread replies as runtime operation because they mutate task-backed conversation history and
  can influence active runtime coordination through mailbox forwarding.
- Preserve existing resource boundaries after capability checks; Team ownership, actor scope,
  thread lookup, task lookup, mention normalization, and mailbox forwarding still run after the user
  capability gate.
- Keep task list/detail/message-list, message search, and compile-preview for a later read/preview
  classification pass.

## Validation

```bash
cargo test -p agenthub api::teams::tests::teams_router_http_contract -- --nocapture
cargo test -p agenthub api::authz::tests::api_code_does_not_bypass_capability_authz_for_human_roles -- --nocapture
cargo fmt -p agenthub -- --check
git diff --check
```

## Follow-Ups

- Continue auditing other API route clusters for authentication-only authorization. Team
  read/preview routes are covered by
  [2026-07-21 Team Read Preview Capability Gate](2026-07-21-team-read-preview-capability-gate.md),
  and the shared-thread ensure mutation is covered by
  [2026-07-21 Team Shared Thread Capability Gate](2026-07-21-team-shared-thread-capability-gate.md).
