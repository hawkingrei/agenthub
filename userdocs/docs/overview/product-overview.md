---
sidebar_position: 1
---

# Product Overview

AgentHub is a self-hosted agent loop toolchain. Its product direction is to let
agents advance tasks through IM, task-list, execution, and Nowledge Mem tools,
using one configured role prompt for each activation.

The loop model is a target design. Automatic activation of offline agents and
the complete Nowledge Mem integration are not yet delivered. Current runtime
controls and installation instructions remain applicable during the transition.

## Core Idea

The target workflow is:

1. Express a goal or provide new information in IM.
2. An eligible trigger activates an agent using its Agent Card and role prompt.
3. The agent reads messages, tasks, and relevant knowledge, then uses tools to advance work.
4. It records progress, evidence, and the next action or wait condition.
5. Its process may exit. Later work activates the same logical agent again.

One activation can include many reasoning and tool steps. Finishing a loop does
not necessarily finish the task. A waiting dependency or handoff can be a valid
loop outcome while the task remains open.

Leader and worker roles remain. The existing `coordinator` role is the leader;
the roles use different prompts on the same execution mechanism. Agent Cards
remain part of startup and discovery, independent of whether a process is running.

IM and tasks retain current execution state. Nowledge Mem supplies scoped prior
knowledge, decisions, and selected learning across loops. It does not replace
task ownership or message-delivery records.

## Why Teams Use It

AgentHub fits especially well when engineering work needs more than a single
interactive shell:

- one task may need several execution episodes and external waits
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

The product direction combines:

- **durable work**: agent identity, tasks, messages, and evidence survive process exit
- **temporary execution**: processes run when work is actionable and may exit after recording an outcome
- **shared knowledge**: Nowledge Mem provides relevant context across loops
- **structured ACP history**: plans, tools, output, and debug data stay
  reviewable after the run

Existing remote execution remains available. Remote loop recovery and remote
Mem credential delivery require separate implementation before parity is claimed.

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
