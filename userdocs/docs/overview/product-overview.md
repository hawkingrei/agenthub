---
sidebar_position: 1
---

# Product Overview

AgentHub is a control plane for AI-agent work. It keeps agent sessions alive,
gives operators one place to inspect and steer them, and adds structured
history instead of relying on raw terminal scrollback alone.

## Core Idea

AgentHub is not just a prompt box for one terminal session. It combines:

- long-lived agent runtimes
- a web control surface for start, stop, reconnect, and review
- structured ACP output rendering
- optional Team workflows with leader and worker roles
- optional remote execution nodes for multi-machine rollout

## When AgentHub Fits Well

AgentHub is a strong fit when you need one or more of these:

- browser disconnect should not stop the task
- multiple operators need a shared control surface
- output should stay replayable and auditable
- work should run in isolated worktrees by default
- one machine is no longer enough for all agent execution

## Choose The Right Workflow

- **Agents**: best for one operator driving one agent session directly.
- **Teams**: best when planning, implementation, and review should be split
  across multiple members.
- **Agent Nodes**: best when execution must move to other machines but the main
  control plane should stay central.
- **OpenAPI**: best when AgentHub must be integrated into scripts, CI, or other
  internal tooling.

## What AgentHub Persists

AgentHub keeps operational state in `~/.agenthub/` by default, including:

- SQLite-backed runtime and history state
- agent configuration
- Team state
- audit and operational records

This is why reconnect, history replay, and deployment operations can happen
without treating the browser as the source of truth.

## Next Reading

- [Feature Overview](./feature-overview.md)
- [Architecture Overview](./architecture-overview.md)
- [Installation and Startup](../getting-started/installation.md)
