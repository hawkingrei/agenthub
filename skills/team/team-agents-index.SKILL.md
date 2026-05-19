---
name: team-agents-index
description: Use at Team session startup or when refreshing shared routing and skill pointers.
---

# Team AGENTS Index

Use this skill at Team session startup or when refreshing role/skill routing for coordinator and worker roles.

Primary references:

- Shared Team baseline index: `skills/team/AGENTS.md`
- Unified runtime template: `skills/team/TEAM_AGENTS.md`

## Responsibilities

- Load canonical Team terms and boundaries before role-specific procedures.
- Enforce human/task boundary:
  - humans may speak in free-form conversation, not only goal/constraint form
  - coordinator interprets conversation input and compiles internal Team tasks
  - coordinator owns canonical Team task creation/management
  - Kanban is the canonical Team task surface; channels remain communication/review lanes
- Enforce channel/thread context split:
  - channel root messages are summary-first entrypoints for one topic
  - thread replies are the full-context lane for detailed evidence, logs, reasoning, and follow-up
  - use `agenthub actor team-thread-open` before treating a root channel message as complete context
  - use `agenthub actor team-thread-reply` for topic-specific deep context
- Enforce shared routing vocabulary from `skills/team/AGENTS.md`:
  - `coordinator-mailbox`
  - `peer-mailbox`
  - `shared-channel`
  - `human-notification`
- Align on six Team workflow phases before mailbox execution.
- Route execution details to role-specific skills instead of duplicating procedure text.
- Keep runtime AGENTS context small by loading only role-required skills.
- Remember two shared runtime capabilities:
  - self-profile updates via `profile_patch_proposal`
  - timed self-reminders via `agenthub actor time-trigger-set`,
    `agenthub actor time-trigger-list`, and
    `agenthub actor time-trigger-cancel`
  - canonical Team reporting-surface selection via `team-reporting-surfaces`
  - canonical Team task governance via `team-task-governance`
  - canonical Team task lifecycle via `team-task-lifecycle`

## Routing

- Coordinator AGENTS index: `team-coordinator-agents-index`
- Worker AGENTS index: `team-worker-agents-index`
- Coordinator orchestration: `team-coordinator-orchestrator`
- Worker execution: `team-worker-executor`
- Team reporting surfaces: `team-reporting-surfaces`
- Team task governance: `team-task-governance`
- Team task lifecycle: `team-task-lifecycle`
- Deliberation quality gate: `team-deliberation-rules`
- Actor mailbox protocol: `team-actor-mailbox`
- Timed self-reminders: `agenthub actor time-trigger-set`,
  `agenthub actor time-trigger-list`, and
  `agenthub actor time-trigger-cancel`

## Startup Checklist

1. Read `skills/team/AGENTS.md` shared index.
2. Resolve member role (`coordinator|worker`) and fill one unified runtime template.
3. Initialize or refresh workspace `AGENTS.md`:
   - use `skills/team/TEAM_AGENTS.md`
   - set `role` and `Active Skills` to role-minimal set only
4. Load role-specific skill set based on current phase:
   - coordinator -> `team-coordinator-orchestrator` (+ optional `team-deliberation-rules`)
   - worker -> `team-worker-executor` (+ optional `team-deliberation-rules`)
5. Load `team-reporting-surfaces` whenever local findings must become visible through task notes,
   mailbox, or channels.
6. Load `team-task-governance` whenever canonical Team task fields, notes, or structured task
   context rules are part of the current work.
7. Load `team-task-lifecycle` whenever canonical Team task review or status transitions are part of
   the current work.
8. Check unfinished TODO items in `TODO.md` and, for concrete project workspaces,
   `.agenthubmemory/TODO.md` before processing inbox.
