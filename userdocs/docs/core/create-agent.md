---
sidebar_position: 1
---

# Create Your First Agent

Open **Agents**, select **Create Agent**, and configure one durable runtime.

## Required Inputs

- **Name**: a human-readable runtime label.
- **Command or preset**: the installed ACP entry point, such as
  `agenthub-acp codex`.
- **Execution node**: `Main Node` or a registered remote node. Only root
  operators can select and manage remote nodes.
- **Workspace mode**:
  - `use_existing` runs in an existing directory.
  - `create_worktree` asks AgentHub to create an isolated Git worktree.
  - `reuse_worktree` attaches an already prepared Git worktree.
- **Workdir**: any execution path reachable by the selected node's service
  account. AgentHub does not restrict which directory you choose, so pick one
  deliberately.

Provider-specific model, thinking, mode, and loop controls are optional. The
available values come from the selected runtime rather than a global fixed
list.

## Choosing a Workspace Mode

Prefer `create_worktree` for feature work, experiments, and concurrent agents.
Use `use_existing` only when direct access to that checkout is intentional and
you understand its current uncommitted state. Use `reuse_worktree` when another
workflow owns worktree and branch creation.

For a remote node with **Default worktree root**, `create_worktree` can derive
the final workdir from that root. Without a node default, provide an explicit
path.

## After Creation

The agent appears in the Agents list in `created` state. Select it to start the
runtime, send instructions, inspect history, stop it, or delete it. Deleting an
agent also removes its managed event history, so treat delete as a deliberate
cleanup action.

## Common Creation Failures

- The configured command is missing from the service user's `PATH`.
- A requested remote node is offline or lacks a valid internal gRPC route.
- `create_worktree` does not have a repository/ref or a writable worktree root.

See [Workdir and Worktree Strategy](./workdir-worktree-strategy.md) before
running multiple agents against the same repository.
