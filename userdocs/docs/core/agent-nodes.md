---
sidebar_position: 2
---

# Agent Nodes and Remote Execution

AgentHub can bind an agent to either the local `Main Node` or a registered
remote Agent Node.

## Why Agent Nodes Matter

Agent Nodes are not just a deployment detail. They let AgentHub keep one
control plane while moving execution closer to:

- the repository or dataset
- the available compute
- the required network boundary
- another machine that should own local worktrees and runtime files

This is also where the actor p2p model matters: mailbox and control traffic can
stay consistent even when the process runs remotely.

## What Agent Nodes Control

Each registered node stores:

- a stable node ID
- a human-readable node name
- an encrypted gRPC target
- an optional TLS server name override
- an optional default worktree root

The node registry is a control-plane view. Runtime state still lives on the
selected execution node.

## Deployment Prerequisites

Before registering remote nodes in the UI, make sure the deployment baseline is
ready:

- the main control plane runs with `internal_grpc.enabled = true`
- every remote node also runs AgentHub with internal gRPC enabled
- the main control plane can reach each remote node over an `https://` gRPC
  target
- the cluster shares the same internal gRPC auth/security policy
- each remote node exposes a filesystem layout that matches its configured
  `Default worktree root`

If these prerequisites are missing, AgentHub now rejects remote-target agent
creation instead of creating records that cannot be controlled later.

## Internal gRPC Also Powers `agenthub actor ...`

The same authority-side internal gRPC control plane is also used by the actor
CLI for:

- `agenthub actor team-members`
- `agenthub actor inbox`
- `agenthub actor ack`
- `agenthub actor send`
- `agenthub actor time-trigger-*`
- `agenthub actor permission-review-respond`

That means `internal_grpc.enabled = true` is no longer only a remote-node
deployment concern. Even on a single machine, these commands need a running
AgentHub authority process with internal gRPC enabled.

For local loopback actor CLI usage, keep `internal_grpc.auth.shared_secret`
explicitly present in `~/.agenthub/config.toml`. The authority server may
persist a generated secret under `cert_dir/auth_secret.txt`, but the CLI client
does not reread that generated file when issuing its own token.

## Actor CLI Batch Operations

The authority-side actor CLI also supports small client-side batch workflows for
high-frequency operator actions:

- `agenthub actor ack --message-id 101 --message-id 102`
- `agenthub actor permission-review-respond --permission-id req-1 --permission-id req-2 --option-id approved`

Batch handling is still sequential on the client side. AgentHub does not expose
a separate batch internal gRPC protocol for these operations.

Single-item calls keep their original JSON object output. Multi-item calls
return a JSON array of per-item responses.

For permission review responses, any session or persistent approval path must
use the request-provided `--option-id` value. `--outcome` currently supports
only `cancelled`.

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

## What Still Stays Central

Even in distributed mode, the main AgentHub instance still owns:

- the operator-facing UI and API
- the node registry
- high-level lifecycle intent
- the main control-plane view of tasks, teams, and coordination

Execution moves, but the control surface does not.

## Actor P2P And Mailbox Delivery

Remote execution is designed to preserve the actor model instead of replacing it
with a separate job runner:

- actor control is relayed over internal gRPC
- mailbox delivery can target remote recipients through the same runtime path
- remote nodes keep their local execution data, but the main node still acts as
  the primary operator-facing control plane

This matters because remote execution should still feel like `AgentHub`, not
like a different product with a different control contract.

## Operator Rollout Flow

For a new remote node:

1. Start the remote AgentHub process with internal gRPC enabled.
2. Confirm its gRPC endpoint is reachable from the main control plane.
3. Register the node from `Agents`.
4. Optionally set `Default worktree root`.
5. Create a remote-target agent and verify the card shows `node:<id>`.
6. Start the agent and confirm output is visible in the main workbench.

## Operational Tips

- Keep node IDs stable and environment-oriented, such as `node-east` or
  `build-fleet-a`
- Use a node-specific default worktree root when nodes do not share the same
  home directory layout
- Leave the node default blank when operators must always choose an explicit
  remote workdir
- Validate one remote-target agent end to end before onboarding a full Team to
  that node
