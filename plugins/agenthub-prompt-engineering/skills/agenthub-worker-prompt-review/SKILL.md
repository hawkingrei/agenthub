---
name: agenthub-worker-prompt-review
description: Review or edit AgentHub Team worker prompts and prompt-linked skills when assignment scope, execution initiative, evidence, or coordinator handoff behavior changes.
---

# AgentHub Worker Prompt Review

Use this skill only for the Team worker role. Do not flatten worker and coordinator prompts into one
template merely because they share infrastructure.

## Shared Gate

Read `../agenthub-team-prompt-review/SKILL.md` completely before editing. It owns the shared
composition, workflow, validation, and evidence rules.

## Role Context

From the AgentHub repository root, then read:

- `crates/agenthub-team-prompts/prompts/default_team_worker_prompt.txt`
- `skills/team/team-worker-executor.SKILL.md`

Read the coordinator prompt only when the proposed change affects a genuinely shared contract.

## Worker Boundary

- Preserve the worker as owner of one assigned execution lane, implementation evidence, blocker
  reporting, and scoped human-visible answers about that lane.
- Do not let a role plugin expand assignment scope, create or mutate coordinator-owned canonical
  tasks, or claim acceptance authority.
- Treat runtime-injected identity, assignment, recovery pointers, and permission gates as
  authoritative over plugin guidance and ambient shell state.
- Keep worker-specific implementation initiative, worktree isolation, evidence, blocker, and handoff
  rules in this role's prompt or managed skill; do not move them into the coordinator role.
