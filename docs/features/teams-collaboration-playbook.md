# Teams Collaboration Playbook Specification

## Problem

AgentHub already has Team role contracts, Actor mailbox contracts, and backend run lifecycle rules.
What is still easy to drift is the operational layer: how leader/worker collaboration should run
across cold start, human interaction, delegation, recovery, and long-horizon context management.
This document defines that operational baseline.

## Scope

- Team collaboration flow and role boundaries.
- Canonical Team/Actor terminology for runtime, API, and prompts.
- Team startup/restart behavior and failure handling expectations.
- Mailbox communication policy and message envelope constraints.
- Conversation event-bus carrier contract and routing normalization.
- MCP-first enforcement policy for Team sessions.
- Context/memory layering and promotion rules.
- Membership change and member identity-card synchronization.

## Non-Goals

- Replacing existing API schemas for run/step storage.
- Defining provider-specific prompt copy.
- Prescribing one UI visual style implementation.

## Architecture

### 1) Team Objective And Collaboration Phases

A Team exists to combine heterogeneous model strengths for one complex goal.

Minimum composition:

- exactly one `leader`
- one or more `worker` members

Operational collaboration follows six phases:

1. `team formation`
2. `task analysis`
3. `role assignment`
4. `communication and collaboration`
5. `consensus formation`
6. `result integration`

### 2) Canonical Entities And Identity Mapping

- `team`: collaboration boundary and ownership scope.
- `member`: stable role identity in Team spec (`leader` or `worker`).
- `agent`: runtime process bound to one member.
- `actor`: message identity used by mailbox protocol.
- `run`: execution partition boundary.
- `step`: run-local unit of execution.
- `task`: leader-defined internal work item composed into run/step operations.
- `conversation`: human-facing interaction stream.
- `correlation_id`: chain identity linking one intent across conversation/mailbox/run events.

Human/Task boundary:

- humans do not create internal `task` objects directly;
- humans provide goals/constraints through conversation;
- leader transforms conversation intent into executable internal tasks.

Identity conventions:

- `actor_id` is canonical mailbox identity.
- `agent_id` is compatibility alias for tool/client ergonomics.
- `run_id` remains required for deterministic partitioning and replay.
- `conversation_id` is required for human-facing chat scope.

### 3) Team Lifecycle And Start Semantics

Recommended team lifecycle:

- `creating`: team bootstrap in progress; runtime resources are being prepared.
- `running`: team members are active and coordination loop can execute.
- `stopped`: team exists but no active execution loop.
- `degraded`: team started but one or more required members are unhealthy.
- `failed`: team bootstrap/start failed and requires operator action.

Start semantics:

- service restart must not implicitly start teams;
- execution starts only after explicit `Start Team`;
- leader enters planning/coordination round before delegation.

### 4) Cold-Start Workflow

Leader cold-start:

1. load shared Team AGENTS index (injected builtin `team-agents-index`) and workspace `AGENTS.md` pointer state
2. load leader-specific AGENTS index (injected builtin `team-leader-agents-index`)
3. inspect unfinished items in `TODO.md`
4. detect whether an existing plan can be resumed
5. if no resumable plan exists, create a new planning round

Worker cold-start:

1. load shared Team AGENTS index (injected builtin `team-agents-index`) and role-relevant guidance from `AGENTS.md` + required skills
2. load worker-specific AGENTS index (injected builtin `team-worker-agents-index`)
3. inspect unfinished `TODO.md` entries and, in concrete project repos, `.agenthubmemory/TODO.md`
4. pull mailbox and continue resumable assignments first
5. report status/evidence back to leader

Self-maintenance and deferred follow-up:

- members may patch their own role description/prompt/skill profile through
  `profile_patch_proposal`
- durable identity-card changes should target Team spec; temporary coordination-only changes should
  target the active run override
- members may create one-shot timed self-reminders with `agent_time_trigger_set` and later inspect
  or cancel them with `agent_time_trigger_list` / `agent_time_trigger_cancel`
- timed triggers are for deferred follow-up/review pings, not a replacement for Team task tracking
- members may receive operator-configured `agent_loop` idle-follow-up prompts, but they must treat
  them as continuation nudges for existing work rather than new human requests
- `agent_loop` stays disabled by default and is enabled externally per member; agents should not
  self-enable or retune it unless a human/operator explicitly asks

AGENTS injection matrix:

- shared baseline (both roles):
  - `team-agents-index` -> `skills/team/AGENTS.md`
- unified runtime template:
  - `skills/team/TEAM_AGENTS.md`
- leader role profile:
  - `team-leader-agents-index` (leader skill set only)
- worker role profile:
  - `team-worker-agents-index` (worker skill set only)

Context-size rule:

- runtime `AGENTS.md` must load only role-required and phase-required skills.

### 5) Delegation And Communication Model

- Leader is default human-facing speaker.
- Worker-to-human direct output is exception path and must include rationale.
- Human requests are collected as planning input, not direct task records.
- Delegation payloads should be explicit and testable:
  - objective/task
  - acceptance criteria
  - deadline or completion condition
  - required evidence

Mailbox operational priority (suggested):

- high: blocking failures, urgent coordination, escalation
- medium: member card/discovery broadcasts
- low: routine assignment updates

### 5.1) MCP-First Enforcement Profile

Team collaboration should run with mailbox MCP as the primary communication path:

- Team role sessions must prioritize mailbox MCP tools over shell-based message paths.
- Startup should fail-fast when Team role is enabled but mailbox MCP capability is missing.
- Startup should also fail-fast when required mailbox tools are incomplete (`actor_inbox`, `actor_ack`, `actor_send`).
- Turn loop should stay deterministic:
  - pull inbox -> process -> ack -> send/report -> next pull.
- Team prompt dynamic tail should include an explicit `Allowed actions` block to deny bypass paths.
- Enforcement failures should be visible as structured Team run events and debug-capability snapshots.
- Role default skills remain minimal; deliberation is opt-in by profile.

Detailed enforcement design:

- `docs/features/team-mcp-enforcement.md`

### 5.2) Conversation Event Bus Profile

Conversation lane should use event bus as the realtime chat/timeline carrier, while keeping mailbox
as execution command authority:

- user-facing input should not require user-provided `run_id` or `from_actor_id`;
- backend normalizes sender identity from session and resolves recipients from `@member_id`;
- `conversation_id` is required for chat scope, `run_id` is optional until execution starts;
- `correlation_id` should link one intent chain across chat events and mailbox/run evidence;
- execution command types still require mailbox path (`assignment`, `approval`, `step_action`, execution results).

Detailed event-bus design:

- `docs/features/team-conversation-event-bus.md`

### 6) Context And Memory Layering

Context is workspace-local under `.cache/context` and should be layered:

- `team`: mission/rules/member roster
- `task`: current objectives/constraints
- `journal`: short-horizon observations
- `memory`: distilled long-horizon facts
- `skills`: role execution guidance
- `contracts`: member/peer identity snapshots

Promotion rule:

- recurring/high-value facts move from `journal` to `memory` during coordination rounds.

### 7) Membership And Identity Card Workflow

When adding a worker:

- register member role/identity;
- publish card/capabilities to peers;
- refresh assignments and context contracts.

When removing a worker:

- notify removal;
- clean member references in team/agent contexts;
- rebalance pending work.

Identity-card contract:

- `spec.members[].description` is canonical and should map to
  `/api/agents/:id/.well-known/agent-card` `description`.

### 8) Failure And Recovery Behavior

- Team start should supervise member bootstrap and surface any failure explicitly.
- On member bootstrap failure:
  - bounded retries are recommended (for example, up to three attempts);
  - after retry budget is exhausted, mark Team as `degraded`/`failed` with clear error.
- Unknown member status should be treated as an observability gap and surfaced to operators.

## Contracts

### 1) Actor Envelope Contract

Minimum message envelope:

- `run_id`
- `from_actor_id`
- `to_actor_id`
- `channel`
- `payload`
- `message_id` (server assigned)
- optional `idempotency_key`

Kind projections:

- `from_actor_kind`: `human|agent`
- `to_actor_kind`: `human|agent`

### 2) Human-Facing Contract

- Human-facing Team page is conversation-first.
- Internal task/run/step machinery is debug/operator detail and should not dominate primary flow.
- `Start Team` is exposed as operator action; low-level controls remain in debug surfaces.
- Human operations should target goals/constraints; internal task creation remains leader-owned.
- Conversation message APIs should require `conversation_id` and support mention-only routing without requiring explicit `run_id`.

### 3) Error Surface Contract

- Team creation/start errors must be surfaced with clear human-readable messages.
- API/UI should avoid leaking raw nested JSON blobs as primary error text.
- Structured error chains still belong in logs for diagnostics.

### 4) Context Ownership Contract

- Each member writes only to its own workspace-local `.cache/context`.
- Cross-member sharing uses mailbox/event pointers, not direct filesystem writes.
- Leader workspace should prefer coordination artifacts over feature-code edits.

### 5) Event Stream Contract

- Mailbox traffic should be mirrored into a globally ordered event stream for:
  - audit
  - replay
  - postmortem reconstruction

## Validation Matrix

- process validation:
  - create team (`creating` -> `running`/`failed`) with explicit error visibility
  - manual team start and leader-first planning confirmation
  - full delegation cycle (`assign -> execute -> evidence -> integrate`)
- status/recovery validation:
  - verify member health propagation to Team status (`running|degraded|failed`)
  - verify bounded retry behavior on member bootstrap failures
- messaging validation:
  - verify run-partitioned ordering and replay consistency
  - verify `actor_id` canonical + `agent_id` alias compatibility
- context validation:
  - verify cold-start TODO resume path and `journal -> memory` promotion checkpoints

## Operational Notes

- Keep this playbook aligned with:
  - `docs/features/agents-teams.md`
  - `docs/features/actor-foundation.md`
  - `docs/features/team-mcp-enforcement.md`
  - `docs/features/team-conversation-event-bus.md`
  - `docs/features/backend-runtime-logic.md`
- Prefer small, explicit delegation payloads over broad open-ended assignments.
- Keep mailbox/event stream observability enabled in debug workflows.

## Open Risks

- Some contracts are currently policy-level and need stronger runtime enforcement.
- Membership-card broadcast volume may need throttling in larger teams.
- Team status semantics need consistent backend/frontend mapping to avoid `unknown` confusion.

## Source Notes

- `.info/agent_teams.md`
- `docs/features/agents-teams.md`
- `docs/features/actor-foundation.md`
- `docs/features/backend-runtime-logic.md`
