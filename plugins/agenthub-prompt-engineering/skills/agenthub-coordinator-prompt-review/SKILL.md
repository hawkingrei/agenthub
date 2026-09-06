---
name: agenthub-coordinator-prompt-review
description: Review or edit AgentHub Team coordinator prompts and prompt-linked skills when coordinator authority, delegation, task ownership, or human-facing synthesis changes.
---

# AgentHub Coordinator Prompt Review

Use this skill only for the Team coordinator role. Do not flatten coordinator and worker prompts into
one template merely because they share infrastructure.

## Shared Gate

Read `../agenthub-team-prompt-review/SKILL.md` completely before editing. It owns the shared
composition, workflow, validation, and evidence rules.

## Role Context

From the AgentHub repository root, then read:

- `crates/agenthub-team-prompts/prompts/default_team_coordinator_prompt.txt`
- `skills/team/team-coordinator-orchestrator.SKILL.md`

Read the worker prompt only when the proposed change affects a genuinely shared contract.

## Coordinator Boundary

- Preserve the coordinator as architect, reviewer, canonical task/lifecycle owner, delegation owner,
  and human-facing synthesis owner.
- Do not let a role plugin authorize feature implementation or take worker-owned execution.
- Treat runtime-injected identity, assignment, recovery pointers, and permission gates as
  authoritative over plugin guidance and ambient shell state.
- Keep coordinator-specific research, delegation clearance, task ownership, review, and visible
  synthesis rules in this role's prompt or managed skill; do not move them into the worker role.
