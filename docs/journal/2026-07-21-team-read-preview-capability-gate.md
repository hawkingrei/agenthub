# Team Read Preview Capability Gate

## Summary

Team read-only and compile-preview routes now use the `runtime:inspect` user capability instead of
plain authenticated-user authorization. Device principals are denied before Team ownership, task,
run, message, or preview lookup is attempted.

## Background

The access-control rollout is migrating normal operator routes from coarse authentication-only gates
to product capability gates by route cluster. Team management mutations, runtime controls, run/step
mutations, uploads, mailbox writes, task mutations, and channel thread replies are already covered by
capability gates. The remaining Team read and preview routes still accepted any authenticated
principal before applying resource-scoped checks.

## Scope

- Converted Team list/detail/runtime/shared-thread reads to `runtime:inspect`.
- Converted Team task list/detail/message-list, channel list, message search, and task run
  compile-preview reads to `runtime:inspect`.
- Converted Team run list/detail/snapshot/event/step/inbox reads to `runtime:inspect`.
- Preserved downstream Team ownership, task team-id, run access, actor-id, and mailbox visibility
  checks after the capability gate.
- Left `POST /{id}/shared_thread` for a later management mutation slice because it can create shared
  thread state.

## Key Decisions

- Treat compile-preview as inspection because it compiles existing task and conversation state into a
  preview response without creating a run or mutating Team state.
- Keep `runtime:inspect` as the read capability for Team output, runs, tasks, messages, channels, and
  runtime state so viewers can inspect visible resources while devices remain denied.
- Keep resource checks after capability checks; capability authorization only decides whether the
  principal may attempt the read/preview action.

## Validation

```bash
cargo test -p agenthub api::teams::tests::teams_router_http_contract -- --nocapture
cargo test -p agenthub api::authz::tests::api_code_does_not_bypass_capability_authz_for_human_roles -- --nocapture
cargo fmt -p agenthub -- --check
git diff --check
```

## Follow-Ups

- Continue migrating `POST /{id}/shared_thread` or any other remaining Team management/shared-thread
  mutation routes that still use authentication-only authorization.
