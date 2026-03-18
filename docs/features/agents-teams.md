# Agents And Teams Specification

## Problem

AgentHub Team capabilities (leader/worker roles, actor mailbox, conversation/run/step model,
context continuity) were documented across many timeline notes. Without a unified spec,
terminology and operating expectations drift.

## Scope

- Team operating model and role boundaries.
- Canonical terminology and lifecycle semantics.
- Actor identity and mailbox semantics in Team mode.
- Conversation event-bus communication semantics for Team chat lane.
- Cold-start and context-ownership constraints.

## Non-Goals

- Model-specific prompt copy.
- Full distributed deployment policy.
- Detailed implementation changelog.

## Architecture

### 1) Three-Layer Team Model

1. Human planning layer
- Human collaborates through the shared `Conversation` lane (`all`).
- Human may provide goals, questions, constraints, feedback, approvals, corrections, or free-form
  discussion; human messages do not automatically become Team `task` records.

2. Orchestration layer
- Leader/System interprets shared conversation input and turns agreed execution work into internal
  `task` records.
- A `task` is the agent-facing work object and should be executable without additional human clicks.
- Canonical task creation and lifecycle management belong to leader planning, not direct human task
  authoring.

3. Execution layer
- Creating/accepting a `task` should automatically create a `run`.
- A `run` is the execution record for one task attempt, including status, logs, artifacts, and
  final summary.
- Members execute steps, exchange mailbox messages, and return evidence through the run.

### 2) Role Model

- Leader: architecture/planning/review/synthesis owner.
- Worker: implementation executor, evidence producer.
- Messages without `@mention` target the whole team conversation.
- Speaking policy in shared conversation is leader-first: leader should respond first.
- Worker should speak only when one of these holds: correction of leader error, critical supplement, new finding/evidence, or explicit `@mention`.
- Worker can talk with human directly in the shared team conversation when explicitly mentioned (or when context requires direct clarification), while execution ownership still converges through leader.

### 3) Six-Phase Collaboration Workflow

- `team formation`
- `task analysis`
- `role assignment`
- `communication and collaboration`
- `consensus formation`
- `result integration`

### 4) Team Surface Lanes

- Communication lane:
  - `Conversation` (`all`) is the human-facing lane and remains available without an active run.
  - Human goals/constraints and `@member` coordination requests are authored here.
  - Channels are communication/review lanes, not the canonical task lane.
  - Conversation is a single shared group stream across human, leader, and workers (not per-member isolated chats).
  - Default routing: messages without `@mention` are team-wide with leader-first response priority.
  - Messages with `@member_id` can target one or multiple members and relax worker speaking guardrails.
  - Realtime carrier should use event bus; authoritative persistence remains in `main` DB with outbox relay.
- Execution lane:
  - `Kanban` is the primary task lane.
  - `Runs` is the execution-history lane for run browsing, `Start Team`, and active-run selection.
  - `Agent ACP`, `Overview`, `Events`, `Steps`, `Mailbox`, `Member Console`, and `Debug` are run-scoped lanes.
- Run-scoped lanes should not block conversation; when no run is active they show explicit guidance to return to `Runs`.

### 5) Cold-Start Workflow

- Inject shared Team AGENTS index (`team-agents-index`) to both leader and worker at startup.
- Inject role-specific AGENTS index:
  - leader: `team-leader-agents-index`
  - worker: `team-worker-agents-index`
- Read `AGENTS.md` as index.
- Check unfinished items in `TODO.md` and `.cache/context/todo.md`.
- Load only required skills for current phase.
- Keep decisions/errors in workspace-local context files.

MCP enforcement baseline:

- Team sessions are MCP-first for mailbox communication.
- Team startup should fail-fast if mailbox MCP capability is missing or required mailbox tools are incomplete.
- Team mode denies shell-based mailbox bypass for collaboration traffic.
- Role defaults should keep skill set minimal; optional skills load by explicit profile.
- See `docs/features/team-mcp-enforcement.md` for fail-fast and loop contract details.

### 6) Team AGENTS Injection Matrix

- Shared baseline for both roles:
  - file: `skills/team/AGENTS.md`
  - injected skill: `team-agents-index`
- Unified runtime template for both roles:
  - file template: `skills/team/TEAM_AGENTS.md`
- Leader runtime index:
  - injected skill: `team-leader-agents-index` (apply leader skill profile)
- Worker runtime index:
  - injected skill: `team-worker-agents-index` (apply worker skill profile)

Constraint:

- leader and worker share one template but keep different role-focused active skill sets.
- runtime `AGENTS.md` should include only phase-required skills to control context size.

## Contracts

### 1) Terminology

- `member`: stable identity from `spec.members[].member_id`.
- `agent`: runtime process bound to a member.
- `worker`: member role, not separate entity type.
- `actor_id`: canonical mailbox identity.
- `agent_id`: tool-level alias for `actor_id`.
- `conversation_id`: required human-facing chat scope key.
- `run_id`: mailbox partition and replay boundary.
- `correlation_id`: one intent-chain ID across conversation/mailbox/run events.
- `peer_id`: node identity for mailbox routing.
  - `main`: AgentHub main node.
  - `node`: remote execution node (default non-main peer label).

### 2) Status Values

- Task:
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

### 5) Team Surface Contract

- `Conversation` does not require an active run.
- `Conversation` is not a task list; task creation is an internal Team planning/runtime decision.
- Humans are not required to phrase requests in task form before the Team can act on them.
- `Kanban` is task-first and should show task state plus linked run history/summary.
- Leader owns canonical Team task creation and lifecycle management; workers advance assigned work
  and report progress/blockers promptly so task state remains current.
- `Runs` tab is the only primary entry for run selection/start.
- Run-scoped tabs must use one shared active-run gate policy and one shared fallback guidance pattern.
- Human-facing conversation remains group-visible even when `@mention` is used.
- `@mention` controls response priority and coordination scope, not message visibility.
- Conversation input should allow omission of `run_id`/`from_actor_id`; backend should enrich sender identity and routing from session + mention context.
- Execution-command semantics (`assignment`/`approval`/`step_action`) should still route through mailbox, not event-bus-only transport.
- Manual `compile preview` and `Create Run` actions are debug/advanced tools, not the primary Team workflow.

## Validation Matrix

- `cargo test -p agenthub-team-domain`
- `cargo test -p agenthub -- team::manager::tests`
- `cargo test teams_router_http_contract -- --nocapture`
- `cargo test internal::service::tests -- --nocapture`

## Operational Notes

- Keep Team UX conversation-first; keep run/step internals in debug-oriented surfaces.
- Keep task handling automatic: `task -> run` should happen without human clicks in the normal flow.
- Keep run browsing and start/select operations in the `Runs` tab.
- Keep a shared active-run context header for run-scoped tabs to avoid duplicated controls.
- Team startup may require explicit operator action to bring runtimes online, but once the team is
  ready, new tasks should execute automatically.
- Maintain deterministic run isolation and replay boundaries via `run_id`.

## Open Risks

- Some planning/TODO contracts are prompt-level guidance and not fully runtime-enforced.
- Multi-node actor routing policy remains staged and needs additional hardening.

## Source Journals

- `docs/journal/2026-02-24-team-operating-model-spec.md`
- `docs/journal/2026-02-25-team-runs-tab-and-tab-routing-refactor.md`
- `docs/journal/2026-03-05-main-node-terminology-and-doc-pruning.md`
- `docs/journal/2026-03-05-team-mcp-enforcement-lessons-from-slock.md`
- `docs/journal/2026-03-05-team-conversation-event-bus-contract.md`
