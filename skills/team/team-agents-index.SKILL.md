---
name: team-agents-index
---

# Team AGENTS Index

Use this skill as the shared Team-level startup index for both leader and worker roles.

Primary references:

- Shared Team baseline index: `skills/team/AGENTS.md`
- Unified runtime template: `skills/team/TEAM_AGENTS.md`

## Responsibilities

- Load canonical Team terms and boundaries before role-specific procedures.
- Enforce human/task boundary:
  - humans may speak in free-form conversation, not only goal/constraint form
  - leader interprets conversation input and compiles internal Team tasks
  - leader owns canonical Team task creation/management
  - Kanban is the canonical Team task surface; channels remain communication/review lanes
- Align on six Team workflow phases before mailbox execution.
- Route execution details to role-specific skills instead of duplicating procedure text.
- Keep runtime AGENTS context small by loading only role-required skills.
- Remember two shared runtime capabilities:
  - self-profile updates via `profile_patch_proposal`
  - timed self-reminders via `agent_time_trigger_set/list/cancel`
  - canonical Team task lifecycle via `team-task-lifecycle`

## Routing

- Leader AGENTS index: `team-leader-agents-index`
- Worker AGENTS index: `team-worker-agents-index`
- Leader orchestration: `team-leader-orchestrator`
- Worker execution: `team-worker-executor`
- Team task lifecycle: `team-task-lifecycle`
- Deliberation quality gate: `team-deliberation-rules`
- Actor mailbox protocol: `team-actor-mailbox`
- Timed self-reminders: actor MCP tools `agent_time_trigger_set`, `agent_time_trigger_list`, `agent_time_trigger_cancel`

## Startup Checklist

1. Read `skills/team/AGENTS.md` shared index.
2. Resolve member role (`leader|worker`) and fill one unified runtime template.
3. Initialize or refresh workspace `AGENTS.md`:
   - use `skills/team/TEAM_AGENTS.md`
   - set `role` and `Active Skills` to role-minimal set only
4. Load role-specific skill set based on current phase:
   - leader -> `team-leader-orchestrator` (+ optional `team-deliberation-rules`)
   - worker -> `team-worker-executor` (+ optional `team-deliberation-rules`)
5. Load `team-task-lifecycle` whenever canonical Team task creation, review, or status transitions
   are part of the current work.
6. Check unfinished TODO items in `TODO.md` and, for concrete project workspaces,
   `.agenthubmemory/TODO.md` before processing inbox.
