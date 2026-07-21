# Team Run Step Capability Gate

## Summary

Team run lifecycle and run step mutation routes now use the `runtime:operate` user capability instead
of plain authenticated-user authorization. Operators can create and mutate runs for their own Teams;
viewers are denied before run or step state changes are attempted.

## Background

The access-control rollout is migrating normal operator routes from coarse authentication-only gates
to product capability gates by route cluster. Top-level Team runtime controls already use
`runtime:operate`; this slice applies the same runtime-operation boundary to Team run lifecycle and
run step mutations.

## Scope

- Converted Team run create, cancel, resume, restart, and context flush routes to
  `runtime:operate`.
- Converted Team run step submit, start, complete, fail, input-required, and resume routes to
  `runtime:operate`.
- Added router coverage proving a `viewer` user is denied with `runtime:operate required`.
- Added router coverage proving an `operator` user can create a run, submit a step, start the step,
  and cancel the run for an owned Team.

## Key Decisions

- Treat run lifecycle and step mutation endpoints as runtime operation because they change execution
  state and can trigger downstream runtime work.
- Preserve existing resource boundaries after capability checks; Team ownership, run access, and
  step/run membership checks still run before domain mutations.
- Leave read-only Team run inspection, Team task mutations, Team mailbox actions, and upload routes
  for separate classification slices.

## Validation

```bash
cargo test -p agenthub api::teams::tests::teams_router_http_contract -- --nocapture
cargo test -p agenthub api::authz::tests::api_code_does_not_bypass_capability_authz_for_human_roles -- --nocapture
cargo fmt -p agenthub -- --check
git diff --check
```

## Follow-Ups

- Continue classifying Team read-only run inspection routes.
- Continue migrating Team task, mailbox, and upload mutation routes to explicit capability gates.
