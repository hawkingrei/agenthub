---
name: team-worker-agents-index
---

# Team Worker AGENTS Index

Use this skill as the worker-specific AGENTS index initializer.

Primary references:

- Shared baseline: `skills/team/AGENTS.md`
- Unified runtime template: `skills/team/TEAM_AGENTS.md` (worker profile)

## Responsibilities

- Maintain worker workspace `AGENTS.md` as execution index.
- Maintain project-local durable memory under `.agenthubmemory/` when operating inside a concrete
  repository.
- Keep assignment scope, acceptance criteria, evidence pointers, and blockers concise.
- Keep worker updates routed to leader unless explicit escalation policy applies.
- Keep pointers for self-profile maintenance (`profile_patch_proposal`) and timed follow-up
  reminders (`agent_time_trigger_set/list/cancel`) when they are active.

## Startup Checklist

1. Read shared baseline (`skills/team/AGENTS.md`).
2. Initialize or refresh workspace `AGENTS.md` from `skills/team/TEAM_AGENTS.md`.
3. Set `role=worker` and load minimal `Active Skills`:
   - `team-worker-executor` (role execution skill)
   - `team-actor-mailbox`
   - add `team-deliberation-rules` only when needed
4. If this is a concrete project workspace, ensure `.agenthubmemory/{TODO.md,journal/,note/}`
   exists and use it as the durable memory root.
5. Check `TODO.md` and, when present, `.agenthubmemory/TODO.md` before mailbox rounds.
