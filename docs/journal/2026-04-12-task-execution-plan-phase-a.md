# Task Execution Plan Phase A

## Summary

This change starts the additive `task -> execution_plan.steps[] -> run step` path without changing
the existing Team task-first contract or database schema.

Phase A focuses on three things:

- define a typed `task.context.execution_plan.steps[]` contract;
- validate that contract at the Team manager boundary so all internal task-write paths share the
  same safety checks;
- let task compile preview prefer leader-authored task execution steps over default team-spec step
  generation.

## Contract

Task context may now carry:

- `execution_plan.steps[]`
  - `step_key`
  - `member_id`
  - `depends_on`
  - `goal`
  - `acceptance`
  - `execution.mode`
  - `execution.max_rounds`

Execution modes:

- `single_pass`
- `reconcile_loop`

`reconcile_loop` steps currently require:

- non-empty `goal`
- at least one non-empty `acceptance` item
- `max_rounds` within `1..=32` when specified

The manager also validates:

- referenced `member_id` exists in `spec.members`
- unique `step_key`
- valid `depends_on`
- no dependency cycles

## Behavior

- Task creation and task context updates now reject invalid execution plans early.
- Compile preview now prefers `task.context.execution_plan.steps[]` when present instead of falling
  back to default team-spec steps.
- Runtime materialization stays additive on top of the existing run/step path; the remaining
  follow-up is not step materialization itself, but more autonomous reconcile-loop execution on top
  of the stable lifecycle API.

## Phase B Bridge Progress

An additive runtime bridge now exists:

- compile preview carries `goal`, `acceptance`, and `execution` metadata in `step_template`;
- `create_run(...)` materializes run-local `team_steps` from:
  - explicit run-input `step_template`, or
  - linked-task `execution_plan.steps[]` when no run-input `step_template` is present;
- materialized `team_steps.input_json` now carries:
  - `task_execution_step.goal`
  - `task_execution_step.acceptance`
  - `task_execution_step.execution`

This keeps the current Team step lifecycle API stable while making leader-authored task steps
observable by run-scoped execution.

Still not done:

- there is not yet a dedicated step-round executor that interprets `reconcile_loop` as a bounded
  multi-round worker execution strategy;
- step completion / blocked / review transitions still use the existing lifecycle APIs and do not
  yet enforce round-level reconcile semantics automatically.

## Step Reconcile Runtime Progress

The existing step lifecycle path now understands `reconcile_loop` metadata in an additive way:

- `start_step(...)` and `resume_step(...)` increment `task_execution_step.round_state.current_round`
  and emit `step_reconcile_round_started`.
- `set_step_input_required(...)`, `complete_step(...)`, and `fail_step(...)` persist
  `latest_status`, `latest_outcome`, and `latest_summary` back into
  `task_execution_step.round_state` and emit `step_reconcile_round_finished`.
- This uses the existing `team_steps.input_json` surface rather than adding a new round table.

Current limitation:

- the runtime now records reconcile rounds, but it still depends on explicit external
  `start/input_required/resume/complete/fail` transitions.
- There is still no autonomous worker-side reconcile executor that consumes those step contracts
  and drives rounds without an external transition caller.

## Reconcile Continue Progress

The Team runtime now has a step-scoped `continue` transition for `reconcile_loop` execution:

- `continue_step(...)` is valid only for a `working` reconcile-loop step;
- the transition finishes the current round, advances `current_round`, keeps the step in
  `working`, and persists the provided round output as the latest step output;
- the manager emits:
  - `step_continued`
  - `step_reconcile_round_finished` with `status = "continued"`
  - `step_reconcile_round_started` for the next round
- the internal gRPC step-transition path now accepts `action = "continue"` and auto-nudges the
  worker session again through the existing reconcile prompt injection path.

Guardrail:

- once `current_round` reaches `execution.max_rounds`, `continue_step(...)` rejects the transition;
  the worker must then choose `complete`, `input_required`, or `fail` explicitly.

## Worker CLI Bridge Progress

Workers now have a concrete actor-facing command path for step decisions:

- `agenthub actor team-step-transition` routes to internal gRPC `TransitionStep`;
- `agenthub actor team-step-decision` accepts one structured decision JSON object and translates it
  into the same transition call;
- when no explicit decision payload is passed, `team-step-decision` now reads the canonical
  workspace file `.agenthubmemory/step-decision.json`;
- it supports `start`, `continue`, `complete`, `input_required`, `resume`, and `fail`;
- `continue` can carry structured round output while keeping the step in `working`;
- worker-scoped internal tokens may transition only steps whose `member_id` matches the worker
  token `actor_id`;
- leader/orchestrator tokens keep the broader in-scope transition behavior.

This is intentionally still a structured explicit-decision bridge rather than backend inference from
free-form worker text. The current boundary is:

- worker produces a structured round decision;
- backend applies the step transition and auto-nudges the next reconcile prompt when needed.

The worker-facing contract is now narrower and more stable:

- reconcile prompts and worker skill guidance both point to the canonical workspace file
  `.agenthubmemory/step-decision.json`;
- the prompt includes a concrete decision JSON template with `action`, `output.summary`,
  `output.artifacts`, optional `input.question`, optional `reason`, and optional `error_text`;
- the actor CLI execute layer now has focused regression tests for default decision-file loading and
  missing-file diagnostics.

## Round Result Artifact Progress

Reconcile-loop round results now reuse the existing Team context-artifact persistence path instead
of living only in `team_steps.output_json` and ephemeral run events:

- `continue_step(...)`, `set_step_input_required(...)`, `complete_step(...)`, and `fail_step(...)`
  now attempt to persist a `reconcile_round_result` artifact under the member runtime workspace
  `.cache/context/run/<run_id>/`;
- persisted artifact payloads include:
  - `run_id`
  - `step_id`
  - `step_key`
  - `member_id`
  - `session_id`
  - `round`
  - `status`
  - `summary`
  - optional `output`
  - optional `input`
  - optional `reason`
  - optional `error_text`
- step-level run events now carry `artifact_pointer` plus `artifact_offload_status` for reconcile
  round finish/decision events, so downstream consumers can treat the artifact as the canonical
  round ledger instead of depending on inline large payload echoes.

This tightens the autonomous reconcile-loop path in two ways:

- worker-produced round decisions now leave a stable filesystem-backed audit trail per round;
- backend follow-up nudges can rely on a durable round-result contract even when the output is too
  large or too structured to keep duplicating inline.

## Validation

Focused validation for this phase should cover:

- invalid reconcile-loop plans are rejected during task creation
- invalid member references are rejected during task context patch
- compile preview uses leader-authored task execution steps when present
- run creation materializes run-local steps from preview `step_template`
- run creation falls back to linked-task `execution_plan.steps[]` when run input omits `step_template`
- reconcile `continue` advances the round without forcing a leader-side `resume`
- internal `transition_step(action = "continue")` keeps the step in `working` and requests the
  next reconcile prompt
- reconcile round result artifacts persist for `continue`
- reconcile round result artifacts persist for `input_required`
