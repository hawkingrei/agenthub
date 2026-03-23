---
sidebar_position: 1
slug: /
---

# AgentHub Docs

AgentHub is a single-binary control plane for long-lived AI agents, Team
workflows, and optional remote execution nodes.

## What AgentHub Gives You

Most agent tools are optimized for one terminal and one short-lived session.
AgentHub is built for the operational side of agent work:

- keep agent sessions alive after the browser tab closes
- inspect structured ACP history instead of relying on raw scrollback
- manage Team workflows with explicit shared conversation and task surfaces
- route execution to remote nodes when one machine is not enough
- keep runtime state, history, and operator control in one place

## Core Surfaces

AgentHub has four primary user-facing surfaces:

- **Agents**: create, start, stop, reconnect, and inspect single-agent runs
- **Conversation and ACP timelines**: review plans, tool calls, command output,
  and debug events as structured history
- **Teams**: coordinate leader/worker collaboration with shared conversation and
  task tracking
- **Agent Nodes**: extend execution onto remote machines without moving the
  control plane

## Fast Path

- New to AgentHub:
  - [Product Overview](./overview/product-overview.md)
  - [Feature Overview](./overview/feature-overview.md)
  - [Architecture Overview](./overview/architecture-overview.md)
- Ready to run locally:
  - [Installation and Startup](./getting-started/installation.md)
  - [Configuration Basics](./getting-started/configuration-basics.md)
  - [First Task Walkthrough](./getting-started/first-task-walkthrough.md)
- Working with Teams or deployment:
  - [Team Workbench](./advanced/team-workbench.md)
  - [Agent Nodes and Remote Execution](./core/agent-nodes.md)
  - [Deployment Overview and Topology](./deployment/overview-and-topology.md)

If you want the shortest path to a working local setup:

1. [Installation and Startup](./getting-started/installation.md)
2. [Configuration Basics](./getting-started/configuration-basics.md)
3. [Login and Access](./getting-started/login.md)
4. [First Task Walkthrough](./getting-started/first-task-walkthrough.md)

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

## Recommended Reading Paths

If you are evaluating AgentHub:

1. [Product Overview](./overview/product-overview.md)
2. [Feature Overview](./overview/feature-overview.md)
3. [Architecture Overview](./overview/architecture-overview.md)

If you want to start using AgentHub today:

1. [Installation and Startup](./getting-started/installation.md)
2. [Configuration Basics](./getting-started/configuration-basics.md)
3. [Login and Access](./getting-started/login.md)
4. [First Task Walkthrough](./getting-started/first-task-walkthrough.md)
5. [Create Your First Agent](./core/create-agent.md)

If you operate shared or Team environments:

1. [Team Workbench](./advanced/team-workbench.md)
2. [Agent Nodes and Remote Execution](./core/agent-nodes.md)
3. [Deployment Overview and Topology](./deployment/overview-and-topology.md)
4. [Production Checklist](./deployment/production-checklist.md)
5. [Troubleshooting](./operations/troubleshooting.md)

## Keep Going

- Want the single-agent workflow first:
  [Create Your First Agent](./core/create-agent.md)
- Want to understand session persistence and inspection:
  [Run and Interact](./core/run-and-interact.md),
  [View Output](./core/view-output.md),
  [Session Lifecycle](./core/session-lifecycle.md)
- Want shared multi-agent collaboration:
  [Team Workbench](./advanced/team-workbench.md)
