# Agents And Teams Specification

## Problem

AgentHub Team capabilities (leader/worker roles, actor mailbox, conversation/run/step model,
context continuity) were documented across many timeline notes. Without a unified spec,
terminology and operating expectations drift.

## Scope

- Team operating model and role boundaries.
- Canonical terminology and lifecycle semantics.
- Actor identity and mailbox semantics in Team mode.
- Cold-start and context-ownership constraints.

## Non-Goals

- Model-specific prompt copy.
- Full distributed deployment policy.
- Detailed implementation changelog.

## Architecture

### 1) Three-Layer Team Model

1. Human planning layer
- Human collaborates through `Conversation` under a `Main Task`.
- Human provides goals/constraints; humans do not create internal Team `task` objects directly.

2. Orchestration layer
- Leader compiles planning output into executable `Run` + `Step` graph and internal tasks.

3. Execution layer
- Members execute steps, exchange mailbox messages, and return evidence.

### 2) Role Model

- Leader: architecture/planning/review/synthesis owner.
- Worker: implementation executor, evidence producer.
- Leader answers human planning questions directly; worker outputs flow back via leader.

### 3) Six-Phase Collaboration Workflow

- `team formation`
- `task analysis`
- `role assignment`
- `communication and collaboration`
- `consensus formation`
- `result integration`

### 4) Cold-Start Workflow

- Inject shared Team AGENTS index (`team-agents-index`) to both leader and worker at startup.
- Inject role-specific AGENTS index:
  - leader: `team-leader-agents-index`
  - worker: `team-worker-agents-index`
- Read `AGENTS.md` as index.
- Check unfinished items in `TODO.md` and `.cache/context/todo.md`.
- Load only required skills for current phase.
- Keep decisions/errors in workspace-local context files.

### 5) Team AGENTS Injection Matrix

- Shared baseline for both roles:
  - file: `skills/team/AGENTS.md`
  - injected skill: `team-agents-index`
- Leader-specific runtime AGENTS:
  - file template: `skills/team/TEAM_LEADER_AGENTS.md`
  - injected skill: `team-leader-agents-index`
- Worker-specific runtime AGENTS:
  - file template: `skills/team/TEAM_WORKER_AGENTS.md`
  - injected skill: `team-worker-agents-index`

Constraint:

- leader and worker do not share one identical runtime `AGENTS.md`;
- they share baseline terms but keep different role-focused AGENTS indexes.

## Contracts

### 1) Terminology

- `member`: stable identity from `spec.members[].member_id`.
- `agent`: runtime process bound to a member.
- `worker`: member role, not separate entity type.
- `actor_id`: canonical mailbox identity.
- `agent_id`: tool-level alias for `actor_id`.
- `run_id`: mailbox partition and replay boundary.

### 2) Status Values

- Main task:
  - `open`, `in_progress`, `completed`, `canceled`
- Run/step:
  - `submitted`, `working`, `input_required`, `completed`, `failed`, `canceled`

### 3) Actor Mailbox

- Required envelope partition: `run_id`.
- Identity projection fields:
  - `from_actor_kind`: `human|agent`
  - `to_actor_kind`: `human|agent`
- `agent_id` aliases are additive; canonical storage remains `actor_id`.

### 4) Context Ownership

- Context is workspace-scoped per member under `.cache/context`.
- Cross-member sharing goes through Team channels (events/mailbox/pointers), not direct filesystem writes.
- Leader workspace should remain an empty coordination workspace by default.

## Validation Matrix

- `cargo test -p agenthub-team-domain`
- `cargo test -p agenthub -- team::manager::tests`
- `cargo test teams_router_http_contract -- --nocapture`
- `cargo test internal::service::tests -- --nocapture`

## Operational Notes

- Keep Team UX conversation-first; keep run/step internals in Debug surfaces.
- Team startup should require explicit operator action to begin execution.
- Maintain deterministic run isolation and replay boundaries via `run_id`.

## Open Risks

- Some planning/TODO contracts are prompt-level guidance and not fully runtime-enforced.
- Distributed actor routing policy remains staged and needs additional hardening.

## Source Journals

- `docs/journal/2026-02-24-team-operating-model-spec.md`
- `docs/journal/2026-02-24-team-role-skill-runtime-spec.md`
- `docs/journal/2026-02-20-team-role-workflow-policy.md`
- `docs/journal/2026-02-18-agent-actor-local-distributed-architecture.md`
