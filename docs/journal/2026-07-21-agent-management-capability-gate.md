# Agent Management Capability Gate

## Summary

Agent creation, deletion, and configuration routes now use the `agents:manage` user capability
instead of plain authenticated-user authorization. Remote-target agent creation also keeps the
`nodes:manage` requirement from the node-management rollout.

## Background

The access-control rollout is migrating user-facing routes by capability cluster. Agent runtime
inspection and runtime operation already moved to `runtime:inspect` and `runtime:operate`; this
slice covers agent lifecycle and configuration management without changing object upload behavior.

## Scope

- Converted local agent create, delete, code-mode, Codex default mode, runtime profile, agent loop,
  and ACP mode/model/config routes to `agents:manage`.
- Kept remote-target create-agent requests behind both `agents:manage` and `nodes:manage`.
- Left agent object/image uploads for a separate owner-scope classification slice; that follow-up is
  now covered by [2026-07-21 Agent Upload Capability Gate](2026-07-21-agent-upload-capability-gate.md).
- Added route coverage proving viewers are denied by `agents:manage`, operators can create local
  agents, and operators still cannot create remote-target agents without `nodes:manage`.

## Key Decisions

- Treat runtime profile, agent loop, code mode, and ACP mode/model/config as agent configuration,
  not runtime operation, because they alter future runtime behavior.
- Keep the remote-node create-agent gate layered rather than replacing it with `agents:manage`.
- Classify upload routes separately because upload routes depend on authenticated user identity,
  object owner scope, and upload metadata.

## Validation

```bash
cargo test -p agenthub api::agents::tests::agent_management_routes_require_agents_manage_capability -- --nocapture
cargo test -p agenthub api::authz::tests::api_code_does_not_bypass_capability_authz_for_human_roles -- --nocapture
cargo fmt -p agenthub -- --check
git diff --check
```

## Follow-Ups

- Continue auditing other API route clusters for authentication-only authorization. Agent
  object/image uploads are covered by
  [2026-07-21 Agent Upload Capability Gate](2026-07-21-agent-upload-capability-gate.md).
