# Agent Runtime Capability Gate

## Summary

Agent runtime mutation routes now use the `runtime:operate` user capability instead of
plain authenticated-user authorization. Time-trigger reads use `runtime:inspect`, preserving the
read/write split introduced by the agent inspect route migration.

## Background

The access-control rollout is converting normal user-facing routes by capability cluster. Agent
read, event, discovery-card, and permission-list routes already moved to `runtime:inspect`; this
slice completes the remaining runtime operation routes that mutate running agent state without
changing agent configuration.

## Scope

- Converted start, stop, input, time-trigger create/cancel, ACP session clear, ACP cancel, and ACP
  permission response routes to `runtime:operate`.
- Converted time-trigger list to `runtime:inspect`.
- Left agent create/delete, uploads, code mode, runtime profile, agent loop, and ACP mode/model/config
  routes on their existing authentication behavior for a later `agents:manage` classification slice.
- Added route coverage proving viewers are denied before runtime-operation resource lookup while
  operators can create/cancel time triggers and viewers can inspect scheduled triggers.

## Key Decisions

- Treat time-trigger create/cancel as runtime operation because they enqueue or revoke future agent
  input delivery.
- Treat time-trigger list as runtime inspection because it exposes scheduled runtime state without
  mutation.
- Keep configuration-like routes out of this slice so the next PR can classify agent management
  behavior without mixing it with runtime operation.

## Validation

```bash
cargo test -p agenthub api::agents::tests::agent_runtime_routes_require_runtime_operate_capability -- --nocapture
cargo test -p agenthub api::authz::tests::api_code_does_not_bypass_capability_authz_for_human_roles -- --nocapture
cargo fmt -p agenthub -- --check
git diff --check
```

## Follow-Ups

- Convert agent create/delete/configuration routes to `agents:manage` in a separate reviewable slice.
- Classify upload routes separately because they combine authenticated user identity, resource
  ownership, and object scope metadata.
