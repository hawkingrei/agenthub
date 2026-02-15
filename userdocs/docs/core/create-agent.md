---
sidebar_position: 1
---

# Create Your First Agent

From the Agents page, use the top form to create a new task/agent.

## Key Inputs

- **Name**: human-readable label for management
- **Command or preset**: execution entry (for example a Codex ACP command)
- **Workdir mode**:
  - `use_existing`: run directly in an existing directory
  - `create_worktree`: create an isolated worktree for this run
- **Workdir**: execution path under your allowed safe paths

## Worktree Strategy Tips

- Use `create_worktree` for isolated feature work or experiments
- Use `use_existing` when you intentionally want to reuse an existing workspace
- Keep repositories clean and branch-aware to reduce merge/rebase friction

## After Creation

The new item appears in the running/history card list. You can:

- Start it immediately
- Open execution view
- Stop or delete it later from agent actions
