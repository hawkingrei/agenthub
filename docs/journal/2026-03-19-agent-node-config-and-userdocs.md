# Agent Node Config And User Docs

## Summary

Added editable node-scoped configuration for `agent_nodes`, centered on an
optional `default_worktree_root`, and updated user-facing documentation for
remote node execution.

## Backend

- extended `agent_nodes` persistence with optional `default_worktree_root`
- added legacy-schema migration support for `agent_nodes.default_worktree_root`
- added `PATCH /api/agent_nodes/:id`
- let remote `create_worktree` agent creation derive a workdir from the
  selected node's `default_worktree_root` when `workdir` is blank

## Frontend

- expanded `Agents` node management with:
  - create-time `Default worktree root`
  - inline node editing and save
  - create-agent placeholder/root switching based on selected node

## User Docs

- added a dedicated `Agent Nodes and Remote Execution` guide
- updated create-agent and worktree strategy guidance
- updated deployment topology guidance for distributed node mode

## Validation

- `cargo check --offline`
- `cargo test --locked init_db_adds_agent_nodes_default_worktree_root_column -- --nocapture`
- `cargo test --locked patch_agent_node_updates_default_worktree_root -- --nocapture`
- `cargo test --locked create_agent_route_uses_remote_node_default_worktree_root_when_blank -- --nocapture`
- `cd web && npm run test -- src/components/agent_node_section.test.tsx src/worktree_defaults.test.ts src/app.runtime_effects.test.tsx`

## Notes

- `cd web && npx tsc --noEmit -p tsconfig.json` still reports pre-existing
  repository-wide frontend type errors outside this change area.
- Chrome DevTools MCP validation was not available in this session.
