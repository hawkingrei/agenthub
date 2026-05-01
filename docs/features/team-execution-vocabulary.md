# Team Execution Vocabulary

## Problem

AgentHub Team runtime already distinguishes `task`, `run`, and `step`, but the boundary between
ownership, execution, retry, resume, and planning is still easy to blur. In particular:

- `task` is the primary work/ownership object;
- `run` is the concrete execution partition and replay boundary;
- `step` is a legacy run-local artifact;
- `attempt` and `round` have appeared in docs and code comments without one canonical definition.

Without a stable vocabulary, retries, waiting/resume loops, run recovery, and UI naming drift into
one overloaded "run" concept.

## Scope

- Canonical definitions for `task`, `attempt`, `run`, `step`, and `round`.
- Ownership boundaries between Team planning surfaces and execution/debug surfaces.
- State-transition guidance for retry, resume, `waiting`, and review loops.
- Naming guidance for runtime, API, docs, and UI follow-up alignment.

## Non-Goals

- Changing current database schema in this document.
- Rewriting all existing runtime/UI surfaces in one change.
- Defining provider-specific ACP session internals.

## Architecture

### 1) Layered Execution Model

AgentHub Team execution should be read as five distinct layers:

- `conversation`
  - Human-facing intent and coordination stream.
- `task`
  - Durable ownership/work-tracking object.
- `attempt`
  - One bounded active execution try for a task.
- `run`
  - One concrete execution partition carrying runtime events and replay state.
- `step`
  - Legacy run-local debug artifact inside a run.

`round` sits beside these layers, not inside them:

- `round`
  - A planning or coordination cycle used by coordinator/worker reasoning.
  - Useful for prompt/memory organization, but not the primary execution-tracking surface for
    operators.

## Contracts

### 1) Task

Definition:

- A `task` is the canonical Team work and ownership object.

Properties:

- long-lived compared with runs;
- owns title, status, topic/context, and assigned member;
- may exist before any execution starts;
- may survive multiple execution retries or review loops.

Task states remain:

- `open`
- `in_progress`
- `waiting`
- `in_review`
- `completed`
- `canceled`

Semantics:

- `waiting` means the task is paused on human/external dependency and should not auto-resume just
  because an agent checked again.
- `in_review` means execution output is ready for review/acceptance; it is not the same as active
  execution.

### 2) Attempt

Definition:

- An `attempt` is one bounded try to actively move a task forward.

Attempt start rule:

- Start a new attempt when a task enters active execution from a non-active task state such as:
  - `open`
  - `waiting`
  - `in_review` when explicit rework begins

Attempt end rule:

- End the current attempt when the task leaves active execution into:
  - `waiting`
  - `in_review`
  - `completed`
  - `canceled`

Properties:

- attempt is task-scoped, not team-global;
- attempt is the right semantic bucket for retry/resume accounting;
- multiple runtime incidents may happen inside one attempt without necessarily creating a new
  attempt.

Practical rule:

- if the team is still pursuing the same active execution push and only the concrete runtime
  partition rotated, keep the same attempt;
- if execution paused and later intentionally resumed from a non-active task state, start a new
  attempt.

### 3) Run

Definition:

- A `run` is one concrete execution partition and replay boundary.

Properties:

- carries `run_id`;
- stores runtime events, mailbox partitioning, and replay/debug history;
- is suitable for ACP/runtime telemetry, event listing, and low-level execution inspection;
- is not the primary ownership surface.

Mapping guidance:

- a task may have zero, one, or many runs over its life;
- an attempt usually has one primary run, but recovery may rotate the concrete run/session while
  keeping the same attempt intent;
- a run may exist only for explicit execution flows, not for every planning or review action.

Run states remain:

- `submitted`
- `working`
- `input_required`
- `completed`
- `failed`
- `canceled`

Semantics:

- `run failed` is execution telemetry, not by itself a new task status category;
- retrying after a run failure requires an explicit task/attempt decision instead of silently
  overloading run history.

### 4) Step

Definition:

- A `step` is a legacy run-local execution artifact kept for debug and compatibility.

Properties:

- nested under a run, not under a task directly;
- useful for legacy backend traces and deep debug surfaces;
- should not be treated as the primary Team planning or ownership object.

Naming rule:

- prefer `task`, `attempt`, and `run` in new user-facing design;
- keep `step` in debug/compatibility paths unless a migration explicitly requires it.

### 5) Round

Definition:

- A `round` is a coordination/planning cycle, usually in coordinator/worker reasoning, not the durable
  execution ledger.

Typical usage:

- cold-start planning round;
- deliberation/research round;
- synthesis/review round.

Naming rule:

- use `round` for reasoning/planning cadence;
- do not use `round` as a synonym for `run` or `attempt` in API/runtime/UI execution surfaces.

## State And Transition Guidance

### 1) Waiting / Resume

- moving a task to `waiting` ends the active attempt;
- checking a waiting dependency without new information keeps the task in `waiting` and does not
  start a new attempt;
- new information plus explicit resume moves the task back to `in_progress` and starts the next
  attempt.

### 2) Review / Rework

- moving a task to `in_review` ends the active attempt;
- accepted review may move directly to `completed` without another attempt;
- explicit rework from `in_review` starts a new attempt.

### 3) Run Recovery

- provider/session restart, force-new-session recovery, or runtime replacement inside the same
  active execution push should not automatically imply a new attempt;
- once the task has already left active execution (`waiting`, `in_review`, etc.), any later
  resumed execution is a new attempt even if some runtime state is reused.

## Surface Mapping

### 1) Primary User/Operator Surfaces

- `Conversation`
  - intent, discussion, and shared human-visible progress
- `Kanban`
  - task ownership and task status
- `Runs`
  - concrete execution partitions and execution history

### 2) Debug Surfaces

- `Agent ACP`, `Events`, `Mailbox`, `Member Console`, `Debug`
  - run-scoped telemetry and diagnostics
- `Steps`
  - legacy/deep debug surface only

### 3) Prompt / Memory Surfaces

- `AGENTS.md`, `TODO.md`, `.cache/context/`, `.agenthubmemory/`
  - may refer to task, attempt, and round
- avoid using `run` as the generic name for all ongoing work in prompt prose

## Naming Guidance

Use these defaults in future changes:

- use `task` for ownership, planning, review, and Kanban state
- use `attempt` for retry/resume accounting and active execution tries
- use `run` for concrete runtime/event/replay partitions
- use `step` only for legacy/debug granularity
- use `round` for planning/deliberation/synthesis cadence

Avoid:

- calling every retry a new `run` when the intended semantic boundary is an `attempt`
- calling planning rounds `runs`
- exposing `step` as the main user-facing work object

## Incremental Alignment Plan

1. Keep the current database/runtime schema stable in the short term.
2. Update docs/prompts to use the canonical vocabulary consistently.
3. Align UI labels and empty-state/help text next, especially where `run` is currently used as a
   generic work-progress term.
4. Only after vocabulary is stable, consider whether runtime/API fields need additive
   `attempt_number` or equivalent projections.

## Open Risks

- Existing runtime tables and UI surfaces still carry older `run/step` language, so mixed wording
  will persist until follow-up alignment work lands.
- Attempt boundaries may need additive runtime fields later if operators need first-class attempt
  history rather than derived interpretation from task transitions.

## Source Journals

- [2026-02-12-a2a-agent-team-phase1.md](../journal/2026-02-12-a2a-agent-team-phase1.md)
- [2026-03-09-teams-runtime-controls.md](../journal/2026-03-09-teams-runtime-controls.md)
- [2026-03-16-team-task-status-run-lifecycle-sync.md](../journal/2026-03-16-team-task-status-run-lifecycle-sync.md)
- [2026-04-10-team-workspace-memory-contract.md](../journal/2026-04-10-team-workspace-memory-contract.md)
