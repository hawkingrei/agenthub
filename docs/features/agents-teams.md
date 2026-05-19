# Agents And Teams Specification

## Problem

AgentHub Team capabilities (coordinator/worker roles, actor mailbox, conversation/run/step model,
context continuity) were documented across many timeline notes. Without a unified spec,
terminology and operating expectations drift.

## Scope

- Team operating model and role boundaries.
- Canonical terminology and lifecycle semantics.
- Actor identity and mailbox semantics in Team mode.
- Conversation event-bus communication semantics for Team chat lane.
- Cold-start and context-ownership constraints.

Canonical execution vocabulary (`task`, `attempt`, `run`, `step`, `round`):

- [team-execution-vocabulary.md](./team-execution-vocabulary.md)

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

2. Task ownership layer
- Coordinator/System interprets shared conversation input and turns agreed execution work into internal
  `task` records.
- A `task` is the primary agent-facing work object.
- `task.assigned_member_id` is the canonical owner slot.
- `task.priority` is the canonical urgency field with four stable values:
  - `p0`: immediate / blocking production or coordination issue
  - `p1`: high-priority work that should be scheduled before normal backlog
  - `p2`: default planned work
  - `p3`: low-priority backlog or follow-up
- Canonical execution tasks must not stay unassigned; if a task is materialized onto Kanban, coordinator
  must choose a concrete owner explicitly instead of relying on implicit scheduling or leaving the
  task owner empty.
- Canonical execution task creation requires an explicit `assigned_member_id`; creating an ownerless
  Kanban task is invalid.
- Explicit ownership changes happen through canonical task updates (`assigned_member_id` assign /
  unassign), not implicit runtime scheduling.
- A task also carries an append-only note journal used for:
  - `comment`: progress or blocker notes
  - `decision`: coordinator/owner decisions
  - `result`: concrete outcome/evidence summaries
- Deliberate task status transitions must include a same-update journal note so lifecycle changes
  remain auditable.
- Human-facing UI / HTTP APIs may display canonical task status and ownership, but they do not
  mutate those fields directly.
- Canonical task creation and lifecycle management belong to coordinator planning, not direct human task
  authoring.
- The normal public Team HTTP surface intentionally exposes task reads, not direct canonical task
  creation; coordinator/runtime task creation flows through internal actor controls instead of
  `POST /api/teams/:id/tasks`.

3. Execution telemetry layer
- `run` and `step` are execution/debug artifacts, not the primary collaboration unit.
- Backend no longer runs a Team step-orchestrator worker in the default runtime path.
- Creating a `task` no longer implies backend dispatch; runs are created only by explicit execution
  flows.
- Task progress should be advanced through canonical task state plus mailbox evidence rather than
  implicit backend step scheduling.

### 2) Role Model

- Coordinator (`coordinator` role id for runtime compatibility): architecture/planning/review/synthesis owner.
- Worker: implementation executor, evidence producer, and proactive execution owner inside assigned lanes.
- Messages without `@mention` target the whole team conversation.
- Shared conversation is coordinator-oriented, not worker-silent:
  - coordinator remains responsible for planning, delegation, and final synthesis
  - worker should speak directly when they are the natural factual owner, have a concrete progress update, or can answer the question faster from first-hand execution context
- Worker initiative is encouraged once work is clearly assigned and actionable, but execution should still stay in active dialogue with the coordinator instead of becoming disconnected parallel work.

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
  - Conversation is a single shared group stream across human, coordinator, and workers (not per-member isolated chats).
  - Default routing: messages without `@mention` are team-wide with coordinator-oriented response priority.
  - Messages with `@member_id` can target one or multiple members and further relax worker speaking guardrails.
  - Realtime carrier should use event bus; authoritative persistence remains in `main` DB with outbox relay.
- Execution lane:
  - `Kanban` is the primary task lane.
  - Human operators do not create canonical Team tasks from `Kanban`; they use `Conversation`
    (`all`) to request work or clarify constraints, then coordinator planning / Team runtime materialize
    tasks onto the board.
  - `Runs` is the execution-history/debug lane for explicit run browsing, `Start Team`, and
    active-run selection.
  - `Agent ACP`, `Overview`, `Events`, `Steps`, `Mailbox`, `Member Console`, and `Debug` are run-scoped lanes.
- Run-scoped lanes should not block conversation; when no run is active they show explicit guidance to return to `Runs`.
- Narrow-screen Team detail routes should behave as two panes instead of a long stacked page:
  - Team rail pane (`# all`, `Kanban`, members)
  - workspace pane (selected surface)
  switched by the header toggle.
- The workspace pane must expose direct primary workflow switches for `Conversation` and `Kanban`
  even when the Team rail is hidden.

### 5) Cold-Start Workflow

- Inject shared Team AGENTS index (`team-agents-index`) to both coordinator and worker at startup.
- Inject role-specific AGENTS index:
  - coordinator: `team-coordinator-agents-index`
  - worker: `team-worker-agents-index`
- Read `AGENTS.md` as index.
- Check unfinished items in `TODO.md`; workers in concrete project workspaces should also check `.agenthubmemory/TODO.md`.
- Worker project memory should live under `.agenthubmemory/`:
  - `.agenthubmemory/TODO.md` for the durable task ledger
  - `.agenthubmemory/journal/` for chronological work logs
  - `.agenthubmemory/note/` for reusable lessons and heuristics
- Coordinator usually starts from an empty coordination workspace and can skip `.agenthubmemory/`.
- Project workdirs are stable workspace identities; ordinary restarts must not move an agent to a
  new project directory just because AgentHub generated a new runtime launch id.
- AgentHub runtime launch ids and ACP provider continuity ids are separate:
  - `agent_sessions.id` tracks one concrete local launch
  - `agent_persistent_sessions.session_id` tracks resumable provider continuity
  - `Force New Session` clears the provider continuity id on purpose; ordinary restart should not
- Team skills are system-managed from role, not configured per member:
  - coordinator effective system skills:
    - `agenthub-actor-runtime`
    - `team-agents-index`
    - `team-coordinator-agents-index`
    - `team-coordinator-orchestrator`
    - `team-actor-mailbox`
  - worker effective system skills:
    - `agenthub-actor-runtime`
    - `team-agents-index`
    - `team-worker-agents-index`
    - `team-worker-executor`
    - `team-actor-mailbox`
- Team spec and human UI must not treat `spec.members[].skills` as an operator-managed contract.
- Legacy `spec.members[].skills` input is ignored and stripped during normalization; public Team
  read APIs also redact any persisted legacy member skill arrays from responses.
- Discovery cards and runtime snapshots should expose effective role-derived skills so operators can
  inspect current capabilities without editing them.
- Load only required phase skills on top of the role-bound system baseline.
- Phase skills such as `team-task-lifecycle` and `team-deliberation-rules` remain separately
  loadable; they are not part of the mandatory role-bound system skill set.
- Keep decisions/errors in workspace-local context files.
- Agents may self-maintain their own role profile through `profile_patch_proposal`:
  - `target="team"` updates the durable member identity/card baseline
  - `target="run"` updates run-scoped temporary overrides
  - profile patch scope is limited to prompt/description identity fields; it does not add or remove
    Team skills
  - agents must only patch their own member profile, not another member's
- Agents may schedule one-shot deferred follow-ups via `agent_time_trigger_set`, inspect them via
  `agent_time_trigger_list`, and cancel them via `agent_time_trigger_cancel`; fired triggers arrive
  later as ACP prompts back to the same agent.
- Agents may also run an operator-controlled `agent_loop` idle watchdog:
  - it is disabled by default
  - a human/operator enables it externally per agent
  - it injects a configured ACP follow-up prompt only after a silence timeout
  - injected loop prompts are follow-up nudges for the current task, not new human intent
  - enabling/disabling or updating loop settings must not block normal Team profile/task flows
- Team ACP permission review is mailbox-first:
  - worker-originated ACP permission requests should prefer a non-requester agent reviewer first;
    if another worker is available, route there before falling back to coordinator
  - when multiple non-requester agent reviewers are available, permission review should prefer an
    idle reviewer (no recent non-user ACP output) before interrupting an actively producing agent
  - coordinator-originated ACP permission requests route to an automatically selected subordinate worker reviewer
  - requester must never review its own request; only the current automatically assigned reviewer should see the approval action
  - approval/rejection should be treated as ACP-side review control flow rather than normal peer mailbox work
  - if agent review is unavailable or times out, the system posts a human-review request into
    `Conversation` (`all`)
  - when a new human-review card lands in the Team conversation UI, the browser should emit a
    short local alert tone so the operator notices the fallback immediately
  - human review stays valid in parallel and must not block the original Team workflow

MCP enforcement baseline:

- Team sessions are CLI-first for mailbox/task coordination through `agenthub actor ...`.
- Team startup should fail-fast if the actor CLI coordination capability is missing or the runtime actor env is incomplete.
- Team mode denies ad-hoc mailbox bypass for collaboration traffic.
- Role defaults should keep the system skill set minimal; phase-specific skills may be activated by
  runtime/prompt guidance, but member-authored skill lists are not part of the public Team contract.
- See `docs/features/actor-foundation.md` for CLI-first coordination and mailbox contract details.

### 6) Team AGENTS Injection Matrix

- Shared baseline for both roles:
  - file: `skills/team/AGENTS.md`
  - injected skill: `team-agents-index`
- Unified runtime template for both roles:
  - file template: `skills/team/TEAM_AGENTS.md`
- Coordinator runtime index:
  - injected skill: `team-coordinator-agents-index` (apply coordinator skill profile)
- Worker runtime index:
  - injected skill: `team-worker-agents-index` (apply worker skill profile)

Constraint:

- coordinator and worker share one template but keep different role-focused active skill sets.
- runtime `AGENTS.md` should include only phase-required skills to control context size.
- role-bound system skills come from runtime injection / managed skill install, not from
  `spec.members[].skills`.

## Contracts

### 1) Terminology

For the full execution vocabulary and boundary rules, see
[team-execution-vocabulary.md](./team-execution-vocabulary.md).

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
  - `open`, `in_progress`, `waiting`, `in_review`, `completed`, `canceled`
  - `waiting` means the task is blocked on human/external action and should remain visible without
    asking the assigned agent to continue active execution
  - a later status check with no new information should keep the task in `waiting`, not bounce it
    back to `in_progress`
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
- Worker durable project memory should be kept in project-local `.agenthubmemory/` when operating
  inside a concrete repository.
- `.cache/context/` remains runtime continuity/state storage; it is not the main long-lived project
  notebook.
- Cross-member sharing goes through Team channels (events/mailbox/pointers), not direct filesystem writes.
- Coordinator workspace should remain an empty coordination workspace by default.
- The canonical file/directory contract for `.cache/context/` and `.agenthubmemory/` lives in
  [team-workspace-memory-contract.md](./team-workspace-memory-contract.md).

### 5) Team Surface Contract

- `Conversation` does not require an active run.
- `Conversation` is not a task list; task creation is an internal Team planning/runtime decision.
- Humans are not required to phrase requests in task form before the Team can act on them.
- Human-facing `Conversation` is a bounded recent window, not an unbounded
  archive; operators should use ACP / Runs / other debug lanes for deeper
  history.
- `Kanban` is task-first and should show task state plus linked run history/summary.
- `Kanban` should also surface task priority clearly and allow filtering by priority without forcing
  operators into run/debug tabs.
- Coordinator owns canonical Team task creation and lifecycle management; workers advance assigned work
  and report progress/blockers promptly so task state remains current.
- Human clients may read Team task state from `Kanban`, but canonical task `status` and
  `assigned_member_id` changes remain agent/runtime controls.
- Public Team HTTP clients also do not create canonical tasks directly; they request work in
  `Conversation` and let coordinator/runtime materialize Kanban tasks.
- `task.assigned_member_id` is the long-lived ownership field; canonical execution tasks must be born
  with an owner, and later explicit unassign is an exceptional coordinator repair action rather than
  the default steady state.
- Task detail views should expose the task note journal separately from the human chat transcript so
  operators can inspect TODO/progress/decision history without mixing it into the conversation lane.
- Channels are free-form communication/review lanes; agents should use timed triggers only for
  deferred follow-up and reminders, not as a substitute for canonical Team task tracking in
  `Kanban`.
- The canonical `# all` shared-thread is a dedicated team-level conversation target, not just
  another entry discovered from the paginated Kanban task list.
- Public clients should load and ensure the shared thread through a dedicated Team shared-thread
  contract; hiding it from workspace-task listings is a presentation choice, not a storage/query
  invariant.
- Team surface refresh should tighten around resume/reconnect:
  - selected Team runtime rechecks on focus, visibility restore, and network reconnect
  - active `Kanban` refresh does the same
  - shared `Conversation` re-refreshes immediately on focus, visibility restore, and reconnect in
    addition to SSE and fallback polling
- If legacy data contains multiple shared-thread tasks, backend canonicalization should prefer the
  thread with the newest persisted conversation message; when no shared-thread messages exist yet,
  it should fall back to the oldest created shared-thread record for stability.
- Team ACP permission review requests should auto-route to a non-requester reviewer (`worker -> coordinator`,
  `coordinator -> subordinate worker`), prefer an idle reviewer first when more than one agent candidate
  is available, and fall back to human review in `Conversation` (`all`) when agent review cannot
  complete.
- `Runs` tab is the only primary entry for run selection/start.
- Run-scoped tabs must use one shared active-run gate policy and one shared fallback guidance pattern.
- Team runtime reads should reconcile stale member `running` rows against live runtime handles
  before reporting member/session status so crashed or already-exited members do not keep the Team
  workbench stuck in a stale `running` state.
- `Agent ACP -> Debug` may expose a member-scoped `Force New Session` recovery action:
  - it clears the selected member's persisted ACP session for the active provider
  - restarts only that member runtime
  - other team members keep their current sessions
- The selected Team member workspace also exposes per-agent lifecycle controls:
  - `Start Agent`
  - `Stop Agent`
  - `Delete Agent`
  These act on the selected member's underlying agent record without requiring a Team-wide runtime
  restart.
- Human-facing conversation remains group-visible even when `@mention` is used.
- `@mention` controls response priority and coordination scope, not message visibility.
- When one specific teammate owns the next action, Team coordination should default to
  `to_member` / `to_coordinator` direct mailbox delivery instead of `group_chat`.
- `group_chat` should be reserved for human-visible progress, shared checkpoints, and genuinely
  multi-recipient coordination.
- Large evidence handoffs should be summary-first: send the concise summary in the mailbox/chat
  payload and attach a stable `detail_ref` / artifact pointer for the full content instead of
  pasting large logs or copied context into routine messages.
- Conversation input should allow omission of `run_id`/`from_actor_id`; backend should enrich sender identity and routing from session + mention context.
- Execution-command semantics (`assignment`/`approval`/`step_action`) should still route through mailbox, not event-bus-only transport.
- Manual `compile preview` and `Create Run` actions are debug/advanced tools, not the primary Team workflow.
- Team runtime start failures that originate from a concrete member runtime should surface that
  member-scoped failure to the caller instead of collapsing into a generic opaque team-start
  error.

## Validation Matrix

- `cargo test -p agenthub-team-domain`
- `cargo test -p agenthub -- team::manager::tests`
- `cargo test teams_router_http_contract -- --nocapture`
- `cargo test internal::service::tests -- --nocapture`

## Operational Notes

- Keep Team UX conversation-first; keep run/step internals in debug-oriented surfaces.
- Keep task handling task-first: task state should remain meaningful even when no run exists yet.
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
- `docs/journal/2026-03-20-team-acp-permission-review-routing.md`
- `docs/journal/2026-05-19-team-task-priority-note-governance.md`
