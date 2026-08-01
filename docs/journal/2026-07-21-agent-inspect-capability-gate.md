# Agent Inspect Capability Gate

## Summary

Agent read routes now use the `runtime:inspect` user capability instead of plain authenticated-user
authorization. Viewers can inspect agent state, discovery cards, events, and permission request
lists; device principals are denied before any resource lookup.

## Background

The access-control rollout is migrating normal user-facing routes by capability cluster. Diagnostics,
push subscription, node management, and linker management already moved behind named capabilities.
The next contract-defined cluster is runtime inspection before runtime operation.

## Scope

- Converted agent list/get, discovery-card, event list/get, and permission-list routes to
  `runtime:inspect`.
- Left create/start/stop/input/config/permission-response/time-trigger/upload routes unchanged for
  later `agents:manage` or `runtime:operate` slices.
- Added route coverage proving `viewer` can list agents while `device` is denied across the inspect
  route cluster with `runtime:inspect required`.

## Key Decisions

- Treat discovery-card reads as runtime inspection because they expose agent identity, runtime mode,
  and capability tags without mutating runtime state.
- Keep permission response outside this slice because selecting an option mutates ACP permission
  state and belongs with runtime operation.
- Deny device users before resource lookup so missing-agent paths do not leak existence through
  route-specific error differences.

## Validation

```bash
cargo test -p agenthub api::agents::tests::agent_inspect_routes_require_runtime_inspect_capability -- --nocapture
cargo test -p agenthub api::authz::tests::api_code_does_not_bypass_capability_authz_for_human_roles -- --nocapture
cargo fmt -p agenthub -- --check
git diff --check
```

## Follow-Ups

- Continue the runtime route migration with `runtime:operate` for start, stop, input, ACP cancel,
  permission response, and time-trigger mutations.
- Convert agent create/delete/configuration routes to `agents:manage` in a separate reviewable
  slice.
