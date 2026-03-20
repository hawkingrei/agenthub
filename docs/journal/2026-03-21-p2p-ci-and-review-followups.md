# P2P CI And Review Follow-Ups

## Summary

This change set closes the immediate P2P branch CI regression and applies a small batch of low-risk review follow-ups that improve stability without expanding the PR scope beyond agent-node / remote-target behavior.

## Changes

- Fixed remote-target agent creation SQL placeholder counts in `src/agent/manager.rs`.
  - The remote-target insert branches added new columns but did not update the `VALUES` placeholder count.
  - This caused `create_agent_route_uses_remote_node_default_worktree_root_when_blank` to fail with `16 values for 17 columns`.
- Tightened `delete_agent_node` semantics in `src/agent/manager.rs`.
  - Deleting a missing node now returns a not-found error instead of a silent success.
- Added API coverage for deleting a missing node in `src/api/agent_nodes.rs`.
- Tightened client-side agent node draft validation in `web/src/components/agent_node_section.tsx`.
  - Reject reserved id `main`.
  - Reject invalid node ids that do not match the backend contract.
- Stabilized `availableNodes` / `remoteNodes` memoization in `web/src/components/agent_node_section.tsx` to avoid unnecessary effect churn.
- Restored global descriptors in `web/src/app.runtime_effects.test.tsx` so runtime viewport tests do not leak `localStorage`, `matchMedia`, or RAF overrides into later Vitest files.

## Validation

- `cargo test create_agent_route_uses_remote_node_default_worktree_root_when_blank -- --nocapture`
- `cargo test delete_missing_agent_node_returns_not_found -- --nocapture`
- `cd web && npx vitest run src/components/agent_node_section.test.tsx src/app.runtime_effects.test.tsx`
- `cargo fmt --all --check`
- `cd web && npm run lint -- src/components/agent_node_section.tsx src/components/agent_node_section.test.tsx src/app.runtime_effects.test.tsx src/app.tsx`

## Notes

- These follow-ups stay within the P2P / remote-agent node scope and do not pull unrelated Team UI work back into this branch.
