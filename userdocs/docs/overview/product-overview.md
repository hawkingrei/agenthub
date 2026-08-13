---
sidebar_position: 1
---

# Product Overview

AgentHub is a self-hosted control plane for AI-agent work. It keeps long-lived
agent sessions alive, gives operators one place to inspect and steer them, and
adds structured history instead of relying on raw terminal scrollback alone.

## Core Idea

AgentHub is not just a prompt box for one terminal session. It combines:

- long-lived agent runtimes
- a web control surface for start, stop, reconnect, and review
- structured ACP output rendering
- optional Team workflows with coordinator and worker roles
- optional remote execution nodes for multi-machine rollout

The important shift is that AgentHub treats the browser as a control surface,
not as the place where the agent actually lives. Runtime, history, and
coordination state stay on the backend.

This makes AgentHub closer to an AI agent control plane than to a thin chat
wrapper.

## Why Teams Use It

AgentHub fits especially well when engineering work needs more than a single
interactive shell:

- one operator may need to reconnect to the same long-lived session many times
- multiple people may need to inspect the same run or Team state
- implementation and review may need to be split across multiple agents
- remote execution may need to preserve the same actor mailbox model as local
  execution

## When AgentHub Fits Well

AgentHub is a strong fit when you need one or more of these:

- browser disconnect should not stop the task
- multiple operators need a shared control surface
- output should stay replayable and auditable
- work should run in isolated worktrees by default
- one machine is no longer enough for all agent execution

It is especially useful for engineering teams that want:

- a self-hosted coding-agent workbench
- a multi-agent Team coordination surface
- replayable ACP output for review and debugging
- one control plane across local and remote execution

## Choose The Right Workflow

- **Agents**: best for one operator driving one agent session directly.
- **Teams**: best when planning, implementation, and review should be split
  across multiple members.
- **Agent Nodes**: best when execution must move to other machines but the main
  control plane should stay central.
- **OpenAPI**: best when AgentHub must be integrated into scripts, CI, or other
  internal tooling.

## What Makes AgentHub Distinct

Three properties tend to matter together:

- **long-lived runtime control**: browser refreshes and disconnects do not end
  the work
- **structured ACP history**: plans, tools, output, and debug data stay
  reviewable after the run
- **shared remote-control model**: remote execution uses the dedicated internal
  gRPC control plane instead of introducing another human-facing orchestrator

## What AgentHub Persists

AgentHub keeps operational state in `~/.agenthub/` by default, including:

- a main SQLite control-plane database and per-agent event databases
- agent configuration
- Team state
- audit and operational records
- optional local message and object stores

This is why reconnect, history replay, and deployment operations can happen
without treating the browser as the source of truth.

## Next Reading

- [Feature Overview](./feature-overview.md)
- [Architecture Overview](./architecture-overview.md)
- [Installation and Startup](../getting-started/installation.md)
