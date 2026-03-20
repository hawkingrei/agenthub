# 2026-03-19 Team Task Ownership Contract

## Summary

- Clarified the Team human/task boundary so humans are not forced into a task-shaped input model.
- Made leader ownership of canonical Team task creation/lifecycle explicit across AGENTS/skills and
  injected runtime prompts.
- Tightened worker guidance so workers proactively advance assigned tasks and report progress or
  blockers quickly enough for leader-managed Kanban state to stay current.

## Why

The previous wording was directionally correct but too narrow:

- it implied humans should mainly provide `goals/constraints`, which is too restrictive for actual
  Team conversation;
- it did not state strongly enough that canonical Team task creation/management belongs to leader;
- it did not explicitly tell workers to keep assigned tasks moving and surface status changes fast.

Without this contract, Team runtime behavior and future `leader-driven task creation` work can drift.

## What Changed

- `skills/team/AGENTS.md`
  - human inputs are now explicitly free-form (`goals`, `questions`, `feedback`, `approvals`,
    `corrections`, general discussion);
  - leader owns canonical Team task creation and lifecycle management;
  - channels are communication/review lanes, while Kanban is the canonical task-tracking lane.
- `skills/team/team-agents-index.SKILL.md`
  - clarified that leader interprets conversation input and compiles internal Team tasks;
  - made the channel-vs-Kanban boundary explicit.
- `skills/team/team-leader-orchestrator.SKILL.md`
  - added leader responsibility to interpret free-form human input and create/manage canonical Team
    tasks when explicit tracking is needed.
- `skills/team/team-worker-executor.SKILL.md`
  - clarified that workers do not invent parallel task records;
  - added explicit “keep the task moving” guidance for progress/blocker reporting.
- `docs/features/agents-teams.md`
  - aligned the stable spec with the above contract.
- `crates/agenthub-team-prompts/src/lib.rs`
  - updated injected leader/worker default prompts so live Team sessions receive the same task
    ownership contract, not only the skill docs.

## Follow-up

The contract is now aligned, but implementation is still mixed:

- Team runtime semantics say leader should create/manage canonical tasks;
- current UI still exposes direct `createTeamTask` from the Kanban surface.

Follow-up work should wire this into a real `leader-driven task creation` path so product behavior
matches the contract fully.

## Validation

- `cargo test -p agenthub-team-prompts`
