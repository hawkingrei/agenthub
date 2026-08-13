---
sidebar_position: 2
---

# Feature Overview

This page maps AgentHub's main capabilities to the problems they solve.

## Single-Agent Control Surface

From the `Agents` page you can:

- create, start, stop, reconnect, and delete agents
- choose `use_existing` or `create_worktree`
- point execution at the main node or a remote node
- keep one long-lived session instead of restarting from scratch after every
  browser refresh

Use this path when one person is directly driving one coding or automation
session.

## Structured ACP Output

AgentHub renders ACP sessions as structured timelines instead of plain terminal
logs. That gives you:

- conversation history
- tool-call and command output context
- plan and reasoning segments
- developer-mode debug and raw-event inspection when something looks wrong

Use this when task review, auditability, or deep debugging matters.

ACP debug surfaces can also expose runtime-controlled session actions, such as:

- provider-driven `mode` / `model` selectors
- generic session config updates
- force-new-session recovery when a thread gets stuck

The exact controls depend on the active ACP provider and runtime.

## Team Workbench

`/teams` adds a multi-agent workspace surface with clear role boundaries.

Key concepts:

- `Channels` lead the Team shell, with `# all` as the default shared lane
- the shared channel is stable Team state, not inferred from the visible Kanban
  slice
- `Kanban` is the canonical task lane
- `Execution Runs` and debug surfaces are execution artifacts, not the primary
  planning surface
- `Agent ACP` lets you inspect a specific member deeply when needed

Use this when planning, implementation, and review should be coordinated rather
than collapsed into one session.

## Installable Web App And Push

AgentHub can also behave like an installable web app:

- supported browsers can install it as a standalone app window
- the frontend registers a service worker automatically
- browser push notifications can be attached to the signed-in browser session

AgentHub still prefers fresh server-delivered UI resources on reload; install
support does not turn it into an offline-first shell.

## Agent Nodes

AgentHub can register remote execution nodes and route agent execution to them
over the dedicated internal gRPC transport, while keeping lifecycle control and
actor mailbox delivery in the same control plane.

Use this when:

- repositories or compute live on other machines
- you want one main control plane but many execution targets
- node-specific default worktree roots are required
- actor-to-actor coordination should still work even when execution leaves the
  main machine

## Automation And Integration

AgentHub exposes authenticated OpenAPI discovery endpoints for the subset of
HTTP routes with a published automation contract:

- internal tooling
- typed client generation
- contract checks
- operational automation around agents and Teams

Use this when AgentHub needs to plug into a larger engineering system.
Always generate clients from the exact deployed release because the OpenAPI
document is intentionally incremental.

## Operations And Safety

AgentHub also includes operational guardrails:

- safe-path restrictions for workdirs
- persistent session and history storage
- in-app notifications and browser push delivery
- troubleshooting and recovery guidance

## Next Reading

- [Architecture Overview](./architecture-overview.md)
- [Create Your First Agent](../core/create-agent.md)
- [Team Workbench](../advanced/team-workbench.md)
- [Agent Nodes and Remote Execution](../core/agent-nodes.md)
