---
name: team-task-lifecycle
---

# Team Task Lifecycle

Use this skill whenever Team work must be represented as canonical Kanban tasks.

This skill defines:

- when leader should create a Team task
- which role may advance each task state
- how execution evidence maps into `open / in_progress / in_review / completed / canceled`
- how worker-local TODO state stays aligned with canonical Team task state

Shared routing, human-facing reply rules, and mailbox transport remain canonical in
`skills/team/AGENTS.md` and `team-actor-mailbox.SKILL.md`.

## Canonical Team Task States

- `open`
- `in_progress`
- `in_review`
- `completed`
- `canceled`

State meaning:

- `open`: work is acknowledged but execution has not started yet
- `in_progress`: implementation/research is actively advancing
- `in_review`: worker evidence indicates execution is done and review is pending
- `completed`: review/acceptance passed
- `canceled`: work is intentionally stopped or no longer relevant

## Ownership Contract

- Leader owns canonical Team task creation and lifecycle management.
- Humans may ask for work in free-form conversation; they do not need to phrase requests as tasks.
- Workers do not invent parallel Team task records.
- Workers should proactively surface missing-task situations to leader when execution clearly needs
  Kanban visibility or explicit ownership.

## When Leader Should Create A Task

Create a Team task when at least one of these is true:

- execution work needs explicit ownership
- progress should be visible in Kanban
- multiple checkpoints or review are expected
- the work spans multiple steps, files, or agents
- the human asks for tracked follow-up

Do not force task creation for:

- one-off conversational answers
- pure clarification with no execution work
- transient coordination that does not need history or review

## Leader Task Operations

Leader may:

- create tasks
- assign or reassign owners
- move `open -> in_progress`
- move `in_progress -> in_review`
- move `in_review -> completed`
- move `in_review -> in_progress` when changes are requested
- move any unfinished task to `canceled`
- reopen `completed|canceled -> open` when follow-up is required

Leader review rules:

- do not move a task directly from `in_progress` to `completed` without explicit review/acceptance
- use `in_review` as the default landing state after successful worker execution
- require acceptance evidence before `completed`
- if review fails, return the task to `in_progress` with a concrete change request

## Worker Task Discipline

Workers should:

- treat the leader-owned Team task as the canonical execution record
- keep leader informed when work should move from `open` to `in_progress`
- report evidence as soon as implementation/research materially changes
- ask or signal for `in_review` when acceptance evidence is ready
- never claim the canonical Team task is `completed` on their own authority

Worker completion contract:

- local worker TODO may become `completed` when worker implementation work is done
- canonical Team task should move to `in_review`, not directly to `completed`
- if review requests changes, reopen local worker TODO and continue execution

## Suggested State Mapping

Use this mapping unless a stronger product rule overrides it:

- task created -> `open`
- owner begins execution -> `in_progress`
- linked run succeeds and evidence is ready -> `in_review`
- leader/human review approves -> `completed`
- review requests changes -> `in_progress`
- work intentionally stopped -> `canceled`

## Evidence Requirements

Before moving a task to `in_review`, attach evidence such as:

- changed file paths
- test names or validation commands
- issue links or reproduction notes
- concise summary of what was proven/fixed

Before moving a task to `completed`, confirm:

- review has accepted the result
- acceptance criteria are satisfied
- no known blocker remains open

## TODO Alignment

- Leader TODO tracks coordination/planning work, not a second copy of Team Kanban.
- Worker TODO tracks local execution steps and durable project memory.
- If Team task state and local TODO state diverge, reconcile using mailbox evidence first.
- Keep one authoritative Team task per outcome; compact stale duplicates.

## Guardrails

- Do not create duplicate Team tasks for the same active outcome unless the split is intentional.
- Do not skip `in_review` for non-trivial execution work.
- Do not treat worker confidence as acceptance; use evidence and review.
- Do not let local TODO completion silently imply canonical Team task completion.
