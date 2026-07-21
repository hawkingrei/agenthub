# Team Task Capability Gate

## Summary

Team task mutation routes now use explicit user capabilities instead of plain authenticated-user
authorization. Task creation from channel messages and the public task patch path use
`teams:manage`; task conversation writes use `runtime:operate`.

## Background

The access-control rollout is migrating normal operator routes from coarse authentication-only gates
to product capability gates by route cluster. Team management, runtime controls, run lifecycle,
uploads, and run mailbox operations already use capability checks; this slice covers the remaining
Team task mutation paths without changing read-only task inspection.

## Scope

- Converted channel-message-to-task creation to `teams:manage`.
- Converted the public Team task patch route to `teams:manage` before preserving its canonical
  agent-only rejection.
- Converted task conversation message writes to `runtime:operate`.
- Kept task list, task detail, task message list, message search, thread reply, and compile-preview
  routes unchanged for later read/inspection classification.
- Added router coverage proving viewers are denied before task mutation state changes.
- Added router coverage proving an operator can write task conversation messages for an owned Team
  task and reaches the existing agent-only rejection for public task patch attempts.

## Key Decisions

- Treat task creation from a channel message and public task patch attempts as Team management
  actions because they create or attempt to change canonical Team task records.
- Treat task conversation writes as runtime operation because they mutate task coordination history
  and can forward into active runtime mailboxes.
- Preserve existing resource boundaries after capability checks; Team ownership, task-team
  matching, actor scope validation, idempotency, redaction, and mailbox forwarding semantics still
  run after the user capability gate.
- Leave compile-preview classification out of this mutation slice because it derives a run payload
  preview without writing durable task or runtime state.

## Validation

```bash
cargo test -p agenthub api::teams::tests::teams_router_http_contract -- --nocapture
cargo test -p agenthub api::authz::tests::api_code_does_not_bypass_capability_authz_for_human_roles -- --nocapture
cargo fmt -p agenthub -- --check
git diff --check
```

## Follow-Ups

- Continue classifying Team read-only inspection routes, including task list/detail/message-list,
  message search, thread replies, and compile-preview.
