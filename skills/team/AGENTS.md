# Team Shared AGENTS Index

This file is the shared Team-level AGENTS index injected to both leader and worker
at startup.

## Purpose

- Keep one consistent Team collaboration contract across roles.
- Define stable terms and workflow boundaries before role-specific procedures.
- Ensure both leader and worker start from the same phase model and communication model.

## Canonical Terms

- `team`: collaboration boundary.
- `member`: stable identity in team spec.
- `agent`: runtime process bound to one member.
- `actor_id`: canonical mailbox identity.
- `agent_id`: compatibility alias.
- `run_id`: mailbox partition and replay boundary.
- `task`: internal Team execution unit created by leader planning.

## Human/Task Boundary

- Human provides goals, constraints, priorities, and acceptance expectations through conversation.
- Internal Team `task` objects are created by leader planning.
- Workers execute delegated tasks and report evidence back to leader.

## Team Workflow Phases

1. team formation
2. task analysis
3. role assignment
4. communication and collaboration
5. consensus formation
6. result integration

## Routing Contract

- `AGENTS.md` is index/routing only.
- Detailed procedures are executed from role skills:
  - `team-agents-index`
  - `team-leader-agents-index`
  - `team-worker-agents-index`
  - `team-leader-orchestrator`
  - `team-worker-executor`
  - `team-deliberation-rules`
  - `team-actor-mailbox`

## Startup Contract

- On role startup, load this shared index first.
- Then load role-specific index and execution skills for current phase.
- Before new mailbox work, check unfinished items in:
  - `TODO.md`
  - `.cache/context/todo.md`
