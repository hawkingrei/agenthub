---
name: team-task-governance
description: Use when canonical Team task fields or note journal rules must change.
---

# Team Task Governance

Use this skill when canonical Team task fields, task notes, ownership, priority, or structured task context must stay consistent.

This skill defines:

- how coordinator creates and updates canonical Team tasks
- assignee and priority rules
- how task notes act as the canonical TODO/journal ledger
- how task context stays attached to the canonical task

Shared routing, mailbox transport, and human-facing reply rules remain canonical in
`skills/team/AGENTS.md` and `team-actor-mailbox.SKILL.md`. Shared visibility routing lives in
`team-reporting-surfaces.SKILL.md`.

## Create Contract

- Coordinator owns `agenthub actor team-task-create`.
- Every execution task must be created with an explicit `assigned_member_id`.
- Do not create speculative execution tasks with no owner; choose the concrete member before the
  task enters Kanban.
- Choose title/description around the user-visible outcome, not around one transient command or
  one raw shell session.
- Set priority deliberately at creation time instead of leaving it as an afterthought.

## Ownership Contract

- `assigned_member_id` must map to a real `spec.members[].member_id`.
- Assignee choice should align with the member's identity card and current specialization.
- Reassignment is explicit coordinator action; do not silently let the "real owner" drift away
  from the recorded assignee.
- Avoid keeping active execution tasks unassigned. Explicit unassign should only happen during
  deliberate replanning, cancellation, or archival cleanup.

## Priority Contract

- `p0`: urgent production or user blocker
- `p1`: near-term high-value delivery
- `p2`: default planned work
- `p3`: low-urgency cleanup or backlog

Priority is not cosmetic:

- use it to shape review order, execution order, and filtered task views
- explain meaningful reprioritization in the task note journal
- avoid priority churn with no stated reason

## Task Note Journal Contract

- Treat task notes as the canonical task TODO/journal/evidence ledger.
- Use task notes for durable plan changes, blocker context, decision logs, execution evidence, and
  review handoff summaries.
- Before any deliberate lifecycle transition, append a meaningful task note in the same update flow.
- Note-only updates are still valuable; use them whenever progress, blockers, or evidence changed
  but lifecycle state should stay the same.
- Prefer concise summaries plus stable pointers:
  - changed file paths
  - test names or validation commands
  - issue/PR links
  - artifact or log paths
- Do not paste large logs into task notes when a shorter summary plus pointer is enough.

## Structured Context Contract

- Use task context for structured execution plans, acceptance criteria, links, or machine-readable
  coordination state that should remain attached to the task.
- Prefer additive/merge-style context updates when only part of the context changes.
- When context meaning changes materially, explain that change in the task note journal so humans
  and reviewers can reconstruct why the task shifted.

## Guardrails

- Do not create duplicate active tasks for the same outcome unless the split is intentional.
- Do not change lifecycle state with no note journal context.
- Do not change ownership or priority silently; explain the reason in the task note journal.
