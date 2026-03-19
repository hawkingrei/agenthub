---
sidebar_position: 2
---

# Agent Nodes and Remote Execution

AgentHub can bind an agent to either the local `Main Node` or a registered
remote Agent Node.

## What Agent Nodes Control

Each registered node stores:

- a stable node ID
- a human-readable node name
- an encrypted gRPC target
- an optional TLS server name override
- an optional default worktree root

The node registry is a control-plane view. Runtime state still lives on the
selected execution node.

## Register and Edit Nodes

From the `Agents` page, root operators can:

1. Register a remote node
2. Update its routing fields
3. Set or clear `Default worktree root`
4. Delete the node when no agents still reference it

Non-root users can select already-available execution nodes through existing
agents, but they do not see node-management controls.

## Default Worktree Root

`Default worktree root` is optional and applies to remote `create_worktree`
agents.

- If the selected remote node has a default root, leaving `Workdir` blank in
  `create_worktree` mode is allowed.
- AgentHub derives the actual workdir under that node root.
- If the selected remote node does not define a default root, remote
  `create_worktree` requests must provide an explicit `Workdir`.

This makes it possible to keep each node aligned with its own local filesystem
layout without forcing operators to type paths for every agent.

## Execution Behavior

- `Main Node`: local safe-path and worktree policies apply directly
- Remote node: AgentHub proxies lifecycle control over encrypted gRPC to the
  selected node

When you select a node in the create modal, the `Workdir` placeholder updates
to reflect the effective default root for that node.

## Operational Tips

- Keep node IDs stable and environment-oriented, such as `node-east` or
  `build-fleet-a`
- Use a node-specific default worktree root when nodes do not share the same
  home directory layout
- Leave the node default blank when operators must always choose an explicit
  remote workdir
