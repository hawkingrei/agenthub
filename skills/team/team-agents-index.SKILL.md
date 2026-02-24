---
name: team-agents-index
---

# Team AGENTS Index

Use this skill as the shared Team-level startup index for both leader and worker roles.

Primary references:

- Shared Team baseline index: `skills/team/AGENTS.md`
- Leader runtime template: `skills/team/TEAM_LEADER_AGENTS.md`
- Worker runtime template: `skills/team/TEAM_WORKER_AGENTS.md`

## Responsibilities

- Load canonical Team terms and boundaries before role-specific procedures.
- Enforce human/task boundary:
  - humans provide goals and constraints via conversation
  - leader compiles internal Team tasks
- Align on six Team workflow phases before mailbox execution.
- Route execution details to role-specific skills instead of duplicating procedure text.

## Routing

- Leader AGENTS index: `team-leader-agents-index`
- Worker AGENTS index: `team-worker-agents-index`
- Leader orchestration: `team-leader-orchestrator`
- Worker execution: `team-worker-executor`
- Deliberation quality gate: `team-deliberation-rules`
- Actor mailbox protocol: `team-actor-mailbox`

## Startup Checklist

1. Read `skills/team/AGENTS.md` shared index.
2. Resolve member role (`leader|worker`) and choose role-specific AGENTS template.
3. Initialize or refresh workspace `AGENTS.md`:
   - leader -> `skills/team/TEAM_LEADER_AGENTS.md`
   - worker -> `skills/team/TEAM_WORKER_AGENTS.md`
4. Load role-specific skill set based on current phase.
5. Check unfinished TODO items in `TODO.md` and `.cache/context/todo.md` before processing inbox.
