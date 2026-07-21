# Team Mailbox Capability Gate

## Summary

Team run mailbox operation routes now use the `runtime:operate` user capability instead of plain
authenticated-user authorization. Operators can send and acknowledge mailbox messages for runs they
own; viewers are denied before mailbox state changes are attempted.

## Background

The access-control rollout is migrating normal operator routes from coarse authentication-only gates
to product capability gates by route cluster. Team run lifecycle and step mutations already use
`runtime:operate`; this slice applies the same runtime-operation boundary to run mailbox mutations.

## Scope

- Converted Team run mailbox send, acknowledge, triage, escalate, transfer, and takeover routes to
  `runtime:operate`.
- Kept the run inbox list route unchanged because it is read-only inspection.
- Added router coverage proving a `viewer` user is denied with `runtime:operate required`.
- Added router coverage proving an `operator` user can send and acknowledge a mailbox message for an
  owned Team run.

## Key Decisions

- Treat mailbox send and handling routes as runtime operation because they mutate durable mailbox
  delivery state and can affect active runtime coordination.
- Preserve existing object boundaries after capability checks; run ownership, member scope, actor
  resolution, and message existence checks still run before mailbox mutations.
- Do not close the real multi-node direct-mailbox rollout TODO from this local capability gate; that
  item still requires production or real multi-node evidence.

## Validation

```bash
cargo test -p agenthub api::teams::tests::teams_router_http_contract -- --nocapture
cargo test -p agenthub api::authz::tests::api_code_does_not_bypass_capability_authz_for_human_roles -- --nocapture
cargo fmt -p agenthub -- --check
git diff --check
```

## Follow-Ups

- Continue classifying Team read-only inspection routes.
- Continue migrating Team task mutation routes to explicit capability gates.
