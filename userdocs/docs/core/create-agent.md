---
sidebar_position: 1
---

# Create Your First Agent

From the Agents page, use the top form to create a new task/agent.

## Key Inputs

- **Name**: human-readable label for management
- **Command or preset**: execution entry (for example a Codex ACP command)
- **Execution node**: root-only selector for `Main Node` or a registered remote node
- **Workdir mode**:
  - `use_existing`: run directly in an existing directory
  - `create_worktree`: create an isolated worktree for this run
- **Workdir**: execution path under your allowed safe paths

## Remote Node Notes

- Non-root users create agents on `Main Node` only. The execution-node selector
  and node-management controls are visible to root operators.
- Only root operators can register or edit remote nodes from the Agents page.
- Remote nodes use encrypted gRPC transport for control-plane traffic.
- If a remote node has a configured **Default worktree root**, you can leave
  `Workdir` blank in `create_worktree` mode and AgentHub will derive the final
  workdir under that node-specific root.
- If a remote node does not define a default root, remote `create_worktree`
  requests must provide an explicit `Workdir`.

## Worktree Strategy Tips

- Use `create_worktree` for isolated feature work or experiments
- Use `use_existing` when you intentionally want to reuse an existing workspace
- Keep repositories clean and branch-aware to reduce merge/rebase friction

## After Creation

The new item appears in the running/history card list. You can:

- Start it immediately
- Open execution view
- Stop or delete it later from agent actions
