---
sidebar_position: 1
slug: /
---

# AgentHub Docs

AgentHub is a self-hosted AI agent control plane for long-lived coding agents,
structured ACP timelines, Team workflows, and optional remote execution nodes.

## What AgentHub Gives You

Most agent tools are optimized for one terminal and one short-lived session.
AgentHub is built for the operational side of agent work:

- keep agent sessions alive after the browser tab closes
- inspect structured ACP history instead of relying on raw scrollback
- manage Team workflows with explicit shared channels, Kanban, and execution surfaces
- route execution to remote nodes with actor p2p control and mailbox relay when
  one machine is not enough
- keep runtime state, history, and operator control in one place

If you are evaluating AgentHub as an:

- AI agent control plane
- self-hosted multi-agent platform
- structured ACP workbench
- remote agent execution system

start with the overview pages below.

## Why This Matters

AgentHub is useful when you want the browser to be an operator surface rather
than the runtime itself.

That changes a few things:

- reconnecting to a session should be normal, not a recovery edge case
- output should stay structured and reviewable after the original run ends
- multi-agent collaboration should use explicit shared channels and task surfaces instead of
  ad-hoc shell copy/paste
- remote execution should preserve the same actor and mailbox model instead of
  introducing a separate control plane

## Core Surfaces

AgentHub has four primary user-facing surfaces:

- **Agents**: create, start, stop, reconnect, and inspect single-agent runs
- **Conversation and ACP timelines**: review plans, tool calls, command output,
  and debug events as structured history
- **Teams**: coordinate coordinator/worker collaboration with shared channels,
  Kanban, and execution tracking
- **Agent Nodes and actor p2p**: extend execution onto remote machines without
  moving the control plane, while keeping mailbox/control traffic on the same
  operational model

## Choose Your Path

### Evaluate AgentHub

1. [Product Overview](./overview/product-overview.md)
2. [Feature Overview](./overview/feature-overview.md)
3. [Architecture Overview](./overview/architecture-overview.md)

### Start Running Locally

1. [Installation and Startup](./getting-started/installation.md)
2. [Configuration Basics](./getting-started/configuration-basics.md)
3. [Login and Access](./getting-started/login.md)
4. [First Task Walkthrough](./getting-started/first-task-walkthrough.md)
5. [Create Your First Agent](./core/create-agent.md)

### Operate Teams Or Distributed Execution

1. [Team Workbench](./advanced/team-workbench.md)
2. [Agent Nodes and Remote Execution](./core/agent-nodes.md)
3. [Deployment Overview and Topology](./deployment/overview-and-topology.md)
4. [Production Checklist](./deployment/production-checklist.md)
5. [Troubleshooting](./operations/troubleshooting.md)

## Documentation Map

- **Overview**: what AgentHub is, which surfaces matter, and how the system is
  structured.
- **Getting Started**: install, configure, log in, and complete one first task.
- **Core Workflow**: create agents, choose workdirs/worktrees, run tasks,
  inspect output, and review changes.
- **Advanced Usage**: Team workflows, OpenAPI-based automation, and connection
  recovery.
- **Deployment and Operations**: rollout, production checks, security, and
  troubleshooting.

## After The Basics

- Inspect session persistence and history:
  [Run and Interact](./core/run-and-interact.md),
  [View Output](./core/view-output.md),
  [Session Lifecycle](./core/session-lifecycle.md)
- Learn the workdir and review flow:
  [Workdir and Worktree Strategy](./core/workdir-worktree-strategy.md),
  [Review and Apply Changes](./core/review-and-apply-changes.md)
- Go deeper on Team operation:
  [Team Workbench](./advanced/team-workbench.md)
- Reference materials:
  [API Error Reference](./operations/api-error-reference.md),
  [Configuration Reference](./getting-started/configuration-basics.md)
