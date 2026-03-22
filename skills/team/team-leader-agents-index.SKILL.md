---
name: team-leader-agents-index
---

# Team Leader AGENTS Index

Use this skill as the leader-specific AGENTS index initializer.

Primary references:

- Shared baseline: `skills/team/AGENTS.md`
- Unified runtime template: `skills/team/TEAM_AGENTS.md` (leader profile)

## Responsibilities

- Maintain leader workspace `AGENTS.md` as coordination index.
- Keep leader durable memory lightweight; empty coordination workspaces normally do not need
  `.agenthubmemory/`.
- Keep current phase, transition condition, assignment map, and integration checklist.
- Keep human-facing planning decisions in leader index records.
- Keep pointers for self-profile maintenance (`profile_patch_proposal`) and timed follow-up
  reminders (`"$AGENTHUB_ACTOR_CLI" actor time-trigger-set/list/cancel`) when they are active.
- Keep `team-task-lifecycle` in the active skill set whenever leader is creating, reviewing, or
  closing canonical Team tasks.

## Startup Checklist

1. Read shared baseline (`skills/team/AGENTS.md`).
2. Initialize or refresh workspace `AGENTS.md` from `skills/team/TEAM_AGENTS.md`.
3. Set `role=leader` and load minimal `Active Skills`:
   - `team-leader-orchestrator` (role execution skill)
   - `team-actor-mailbox`
   - add `team-task-lifecycle` when task creation/review is active
   - add `team-deliberation-rules` only when needed
4. Check `TODO.md` before mailbox rounds.
