# Team Operating Model And Terminology Spec

## Problem

Team behavior currently spans multiple feature notes (conversation-first UI, role skills, actor mailbox,
context memory, runtime/startup policies). Without one canonical model, terminology and operator expectations drift:

- `agent`, `worker`, `member`, `actor` are used inconsistently;
- user-facing terms (`Conversation`) and internal terms (`Run`, `Step`) are mixed in UI and docs;
- lifecycle expectations (create/start/execute/recover) are not visible in one place.

This document defines the canonical Team operating model for AgentHub, including terms, state contracts,
and end-to-end workflow phases.

## Scope

- Canonical Team terminology.
- Team object/lifecycle definitions.
- Human-facing flow vs internal execution flow boundaries.
- Team run/step/main-task state machine definitions.
- Leader/worker role contracts and cold-start workflow.
- Actor identity and mailbox partition semantics in Team context.

## Non-Goals

- Replacing subsystem design documents (this spec references them).
- Rewriting Team API payload schemas in this change.
- Defining model-specific prompt text details.
- Finalizing all recovery heuristics (for example multi-attempt auto-restart budgets).

## Architecture

### 1) Core Concepts

Team runtime has three layers:

1. Human planning layer
- Human interacts with Team through `Conversation` under a `Main Task`.
- This layer is the primary user-facing entry.

2. Orchestration layer
- Leader analyzes conversation context and compiles an execution plan.
- Plan materializes as `Run` + ordered/dependent `Step` records.

3. Execution layer
- Member agents execute steps and exchange mailbox messages.
- Evidence is persisted through run events and workspace-scoped context artifacts.

### 2) Canonical Relationships

- One `Team` contains multiple `members` (`spec.members[]`).
- Each member is an `agent` bound to a role (`leader` or `worker`).
- A `Main Task` is a long-lived planning objective for a team.
- A `Conversation` is the human/team message timeline bound to one main task.
- A `Run` is one executable snapshot compiled from planning context.
- A `Step` is one execution unit within a run, assigned to one member.
- An `Actor` is mailbox identity (`actor_id`) used for send/inbox/ack operations.

### 3) Human-Facing Vs Internal Concepts

- Human-facing primary concepts:
  - Team
  - Conversation
  - Start Team
- Internal/debug concepts:
  - Main task internal IDs
  - Run ops
  - Step ops
  - Raw mailbox payloads

Product policy:
- Internal execution concepts must remain available in Debug surfaces.
- Primary `/teams` experience should stay conversation-first.

### 4) Team Collaboration Phases

Team collaboration follows six explicit phases:

1. `team formation`
- Define team mission, leader, workers, and member descriptions.

2. `task analysis`
- Leader and human clarify objective, constraints, acceptance criteria.

3. `role assignment`
- Leader decomposes work and assigns execution responsibilities.

4. `communication and collaboration`
- Members execute, report status/evidence, request clarifications.

5. `consensus formation`
- Leader reconciles outputs/conflicts and forms integrated solution.

6. `result integration`
- Leader delivers final human-facing answer and records key decisions.

### 5) Cold-Start Workflow

All Team members follow TODO-first cold start:

- read `AGENTS.md` as role index;
- inspect unfinished items in `TODO.md` and `.cache/context/todo.md`;
- load only required skills for current phase;
- append decisions/errors in workspace-local context tree.

Role boundary:
- Leader must answer human planning questions directly.
- Leader delegates implementation to workers by default.
- Worker executes delegated tasks and returns evidence to leader.

## Contracts

### 1) Terminology Contract

- `member`: Team spec identity entry (`spec.members[].member_id`), stable across runs.
- `agent`: Runtime process bound to a member.
- `worker`: A member role, not a separate runtime entity type.
- `actor_id`: Canonical mailbox sender/receiver identity.
- `agent_id`: CLI/MCP alias for `actor_id` at tool boundary.
- `run_id`: Mailbox partition key and execution isolation boundary.

### 2) Team/Main Task/Conversation Status Contract

Main task status values:
- `open`
- `in_progress`
- `completed`
- `canceled`

Conversation contract:
- One conversation belongs to one main task.
- Conversation messages persist route and actor identity metadata.

### 3) Run/Step Status Contract

Run status values:
- `submitted`
- `working`
- `input_required`
- `completed`
- `failed`
- `canceled`

Step status values:
- `submitted`
- `working`
- `input_required`
- `completed`
- `failed`
- `canceled`

Transition principles:
- `submitted -> working` on start.
- `working -> completed|failed|input_required` by execution result.
- terminal states are `completed|failed|canceled`.

### 4) Member Lifecycle Display Contract (UI)

Top-level Team member lifecycle buckets:
- `working`
- `idle`
- `stopped`
- `missing`

Normalization guidance:
- runtime `running|working` -> `working`
- runtime `idle` -> `idle`
- runtime `stopped|completed|failed|exited|canceled` -> `stopped`
- unresolved member-agent binding -> `missing`

`unknown` handling rule:
- `unknown` is diagnostic/intermediate and should not be the primary user-facing state.
- UI should either resolve to canonical bucket or expose remediation hint in Debug view.

### 5) Team Create Lifecycle Contract

- Before final `Create Team`, data is draft-only (`status: creating`) in browser-local storage.
- Persisted Team definition is created only at final submit.
- Create-stage failures must be surfaced as explicit user-visible errors.

### 6) Actor Mailbox Contract In Team Mode

Mailbox envelope invariants:
- `run_id` required for partitioning and replay isolation.
- `actor_id` canonical identity field.
- `agent_id` accepted as alias in CLI/MCP.
- payload should include explicit identity kind projection:
  - `from_actor_kind`: `human|agent`
  - `to_actor_kind`: `human|agent`

### 7) Identity Card Contract

- `spec.members[].description` is canonical identity description.
- Member card (`/.well-known/agent-card`) `description` should map from that source.
- Team role prompts and skills should treat the identity card as the member's external profile.

### 8) Context Ownership Contract

- Context memory is workspace-scoped per member.
- Each member writes only to its own `<workspace>/.cache/context/...` tree.
- Cross-member context exchange goes through Team channels (events/mailbox/pointers), not direct filesystem writes.
- Leader workspace should be an empty coordination workspace by default and is reserved for planning/context artifacts instead of feature-code edits.

## Validation Matrix

Expected verification set for this feature area:

- Terminology and status constants:
  - `cargo test -p agenthub-team-domain`
- Team manager run/step state behavior:
  - `cargo test -p agenthub -- team::manager::tests`
- Conversation-first UI behavior:
  - `pnpm -C web exec vitest run src/pages/team_panels.test.tsx src/pages/team_page.runs.test.ts src/pages/team/state.test.ts`
- Member lifecycle strip mapping:
  - `pnpm -C web exec vitest run src/pages/team_member_status_strip.test.tsx`
- Optional end-to-end user flow:
  - `pnpm -C web run e2e:llm-local`

## Operational Notes

- This is the canonical Team operating model. Subsystem docs should align to this terminology.
- Keep `AGENTS.md` concise as index/routing metadata; detailed procedures belong to `SKILL.md`.
- Keep primary Team UX conversation-first and hide internal `run/step` mechanics from default user path.
- Keep Start Team as explicit operator action; do not assume runtime liveness from stale DB status alone.

## Open Risks

- Some recovery contracts are still guidance-level and need stricter runtime enforcement:
  - startup/restart retry budget and escalation policy;
  - hard guarantees around stale runtime status reconciliation under repeated crashes.
- Multiple existing Team feature notes still duplicate partial terminology.
- `unknown` lifecycle states may still leak through edge paths until all mapping points converge.

## Canonical References

- `docs/features/2026-02-22-team-context-memory-architecture.md`
- `docs/features/2026-02-18-agent-actor-local-distributed-architecture.md`

## Superseded Notes

Merged into this spec and removed from `docs/features`:

- `docs/features/2026-02-24-team-operating-model-spec.md`
- `docs/features/2026-02-24-team-operating-model-spec.md`
- `docs/features/2026-02-24-team-operating-model-spec.md`
- `docs/features/2026-02-24-team-operating-model-spec.md`
- `docs/features/2026-02-24-team-operating-model-spec.md`
