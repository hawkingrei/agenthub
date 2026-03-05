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
- Keep current phase, transition condition, assignment map, and integration checklist.
- Keep human-facing planning decisions in leader index records.

## Startup Checklist

1. Read shared baseline (`skills/team/AGENTS.md`).
2. Initialize or refresh workspace `AGENTS.md` from `skills/team/TEAM_AGENTS.md`.
3. Set `role=leader` and load minimal `Active Skills`:
   - `team-leader-orchestrator` (role execution skill)
   - `team-actor-mailbox`
   - add `team-deliberation-rules` only when needed
4. Check `TODO.md` and `.cache/context/todo.md` before mailbox rounds.
