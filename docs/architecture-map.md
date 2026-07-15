# Architecture Map

This page is a second-level index for contributors who want the deeper
technical architecture behind AgentHub instead of the user-facing product
overview in the repository `README.md`.

Use this page when the question is about design boundaries, runtime contracts,
or rollout shape.

## Start Here

- Product and system framing:
  - [features/agents-teams.md](features/agents-teams.md)
  - [features/backend-runtime-logic.md](features/backend-runtime-logic.md)
- Frontend and workspace shell:
  - [features/frontend-design.md](features/frontend-design.md)
  - [features/workspace-unified-ia.md](features/workspace-unified-ia.md)
- ACP and runtime rendering:
  - [features/acp-runtime.md](features/acp-runtime.md)
- Actor and Team coordination:
  - [features/actor-foundation.md](features/actor-foundation.md)
  - [features/team-conversation-event-bus.md](features/team-conversation-event-bus.md)
  - [features/team-channels-threads.md](features/team-channels-threads.md)
- Distributed execution and nodes:
  - [features/agent-nodes.md](features/agent-nodes.md)
  - [features/distributed-node-architecture.md](features/distributed-node-architecture.md)
- Context, memory, and long-running continuity:
  - [features/team-workspace-memory-contract.md](features/team-workspace-memory-contract.md)
- Agent workflow organization:
  - [features/agent-operating-workflows.md](features/agent-operating-workflows.md)

## By Question

### How does AgentHub model agents, Teams, tasks, and execution?

- [features/agents-teams.md](features/agents-teams.md)
- [features/team-execution-vocabulary.md](features/team-execution-vocabulary.md)

### How does Team communication work?

- [features/team-channels-threads.md](features/team-channels-threads.md)
- [features/team-conversation-event-bus.md](features/team-conversation-event-bus.md)
- [features/actor-foundation.md](features/actor-foundation.md)

### How does ACP state, replay, and UI rendering work?

- [features/acp-runtime.md](features/acp-runtime.md)
- related implementation journals in [journal/](journal/)

### How does the shared workspace shell fit together?

- [features/workspace-unified-ia.md](features/workspace-unified-ia.md)
- [features/frontend-design.md](features/frontend-design.md)

### How does remote execution work?

- [features/agent-nodes.md](features/agent-nodes.md)
- [features/distributed-node-architecture.md](features/distributed-node-architecture.md)

### How is long-running memory and context handled?

- [features/team-workspace-memory-contract.md](features/team-workspace-memory-contract.md)

### How are SOPs, skills, checklists, testing, and observability workflows organized?

- [features/agent-operating-workflows.md](features/agent-operating-workflows.md)
- [features/test-regression-guardrails.md](features/test-regression-guardrails.md)
- [features/runtime-diagnostics.md](features/runtime-diagnostics.md)
- [features/pyroscope-profiling.md](features/pyroscope-profiling.md)

## Related Contributor Docs

- Contributor guide: [developer-setup.md](developer-setup.md)
- Documentation guide: [README.md](README.md)
- Active backlog: [todo.md](todo.md)
- Feature-doc standard: [features/README.md](features/README.md)
