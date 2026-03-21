# Agent Manager Store Split

## Summary

Refactored `src/agent/manager.rs` to move schema-capability-aware agent persistence into
`src/agent/manager/store.rs`.

## Motivation

`AgentManager` had accumulated repeated SQL branches for legacy-schema compatibility:

- `source` column present or absent
- `target_node_id` column present or absent

This produced repeated `INSERT`, `UPDATE`, `SELECT`, and row-decoding logic in the manager,
which made the orchestration flow hard to read and increased the cost of future schema changes.

## What Changed

- Introduced `AgentSchemaCaps` to centralize agents-table capability discovery.
- Moved target-node decoding and agent row decoding into the store layer.
- Moved agent create and remote-managed upsert SQL generation into store helpers.
- Moved agent read-side projection selection for `list_agents` and `get_agent` into store helpers.
- Moved `agent_nodes` persistence and legacy-schema branching into
  `src/agent/manager/nodes.rs`.
- Moved Git worktree parsing and matching helpers into
  `src/agent/manager/worktree.rs` so `runtime.rs` can stay focused on startup/shutdown flow.
- Moved agent session lifecycle methods into `src/agent/manager/session.rs`, including:
  persistent ACP session bookkeeping, runtime start/stop orchestration, startup failure recording,
  and running-session reconciliation.
- Moved process/output lifecycle into `src/agent/manager/process.rs`, including:
  output stream persistence/classification, exit watching, and finalized process-exit bookkeeping.

## Result

- `AgentManager` now stays focused on orchestration and runtime decisions.
- Legacy schema compatibility branches are localized in one module.
- Worktree parsing is isolated from runtime/session lifecycle code.
- Session bootstrap and shutdown flow now live in a dedicated module instead of the top-level
  manager file.
- Process I/O and exit finalization are isolated from worktree/runtime preparation logic.
- Future column additions can be handled in the store layer without multiplying manager branches.

## Validation

Targeted regression coverage:

- `create_agent_treats_main_target_node_as_local`
- `create_team_forge_agent_route_rejects_remote_target_on_legacy_schema`
- `list_agents_hides_team_forge_source_agents`
- `remote_agent_grpc_control_starts_inputs_and_lists_events_over_tls`
