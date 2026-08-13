---
sidebar_position: 3
---

# Architecture Overview

This page explains the main runtime pieces behind AgentHub without going into
internal implementation detail.

AgentHub is designed as a self-hosted AI agent control plane with a clear split
between the operator-facing control plane and the machine-local or remote
execution plane.

## High-Level Shape

AgentHub is designed as one main control-plane service:

- one Rust backend process
- one embedded web UI served by the same process
- a main SQLite control-plane database plus per-agent SQLite event stores
- HTTP plus SSE for browser interaction and live updates

That baseline is enough for local and many shared internal deployments while
still preserving a path to remote execution nodes.

## Control Plane Versus Execution Plane

AgentHub keeps these concerns separate:

- the **control plane** owns API handling, auth, session state, Team state, and
  operator-visible history
- the **execution plane** is where agent processes actually run, either on the
  main machine or on remote Agent Nodes

This separation is what lets AgentHub keep one operational model across local
and distributed execution.

## Main Runtime Components

### Control plane

The backend owns:

- authentication and API handling
- agent lifecycle management
- Team runtime coordination
- session and history persistence
- remote-node control when distributed execution is enabled

### Web UI

The embedded SPA gives operators a browser surface for:

- agent management
- session inspection
- Team collaboration
- admin and operational tasks

### ACP runtime

ACP is the structured event model behind conversation, tool calls, debug
surfaces, and history replay. AgentHub persists these events and replays them
through the UI instead of relying only on raw process output.

### Team runtime

The Team model has three layers:

- `Channels` for human-facing shared coordination, with `# all` as the default
  shared lane
- `Kanban` for canonical task state
- `Execution Runs` plus debug surfaces for execution telemetry and deep
  inspection

This keeps collaboration state separate from lower-level execution artifacts.

## Distributed Topology

When one machine is not enough, AgentHub can add remote Agent Nodes:

- the main AgentHub instance stays the control plane
- remote nodes run the same `agenthub` binary
- agent lifecycle control and actor mailbox relay use the dedicated internal
  gRPC transport
- node-local execution data stays on the selected node

This lets operators keep one main UI and API surface while execution happens
elsewhere.

The important architectural point is that remote execution does not introduce a
second human-facing control surface. The actor and mailbox contract stays
consistent whether execution is local or remote.

## Persistence And Reliability

AgentHub stores control-plane state in the main SQLite database and high-volume
agent output in per-agent SQLite databases. Optional message archives, message
body stores, and object stores add separate data paths. This persistence lets
these behaviors remain stable:

- browser refresh or disconnect does not stop the task
- session history can be reopened later
- Team and agent operational state can be audited
- safe-path policy can be enforced consistently

## What Changes Between Modes

- **Single-agent mode**: one operator, one agent, direct control from `Agents`
- **Team mode**: multi-member collaboration with explicit shared conversation
  and task surfaces
- **Distributed mode**: remote execution over internal gRPC with the same main
  control plane

## Next Reading

- [Feature Overview](./feature-overview.md)
- [Deployment Overview and Topology](../deployment/overview-and-topology.md)
- [Agent Nodes and Remote Execution](../core/agent-nodes.md)
