---
name: team-coordinator-orchestrator
description: Use when acting as the coordinator for a Team run.
---

# Team Coordinator Orchestrator

Use this skill when acting as the coordinator for a multi-agent Team run.

## Objectives

- Convert run input into a short, ordered execution plan.
- Interpret human channel input in context, including free-form questions, feedback, approvals,
  and corrections.
- Create and maintain the canonical Team task set when execution tracking is needed.
- When a task needs explicit execution structure, author `task.context.execution_plan.steps[]`
  instead of relying on implicit worker interpretation.
- Delegate concrete, testable tasks to workers via actor mailbox.
- Aggregate worker outputs and produce one final answer.
- Communicate directly with the human actor for planning, decisions, and final delivery.
- Operate as team architect and code reviewer for feature work.
- Supervise worker execution quality, reporting cadence, and evidence freshness.
- Publish integrated progress updates when team state, risks, or findings materially change.
- Surface reusable findings and team lessons to the human/channel when they affect future work.

## AGENTS Index Contract

- Treat workspace `AGENTS.md` as the role-level index and routing source.
- Do not duplicate large procedural detail in `AGENTS.md`; keep details in `SKILL.md`.
- On startup and on phase changes, refresh `AGENTS.md` pointers to the active skills and artifacts.
- Bootstrap from `skills/team/TEAM_AGENTS.md` and set coordinator skill profile when creating coordinator `AGENTS.md`.

## Skill Routing Contract

- Use `team-agents-index` to load shared Team terminology and startup checklist first.
- Use `team-coordinator-agents-index` to load coordinator-specific AGENTS template/rules.
- Use this skill for coordinator planning, assignment, synthesis, and human-facing coordination.
- Use `team-task-governance.SKILL.md` for canonical Team task creation, ownership, priority, note journal, and visibility rules.
- Use `team-task-lifecycle.SKILL.md` for canonical Team task review and status changes.
- Use `team-deliberation-rules.SKILL.md` for cross-option evaluation and consensus discipline.
- Use `team-actor-mailbox.SKILL.md` as source-of-truth for mailbox protocol details (`inbox`/`receive`/`send`/`ack`).
- Treat `task.context.execution_plan.steps[]` as the canonical coordinator-authored execution recipe
  when a task needs explicit step structure.

## Shared Contract Usage

- Routing keys, mention discipline, and human-facing reply rules are shared in `skills/team/AGENTS.md`.
- In planning artifacts and payload examples, always reference teammates by stable `member_id`.
- Treat `spec.members[].description` as the canonical A2A identity-card baseline and verify `/api/agents/:id/.well-known/agent-card` when role ownership is ambiguous.
- Use worker identity cards as the default capability map for delegation and collaboration planning.
- Record any required identity-card update checkpoints in coordinator coordination artifacts instead of duplicating policy here.
- If your own coordinator description/prompt/skill profile needs correction, send `profile_patch_proposal`
  for your own member record; use `target="team"` for durable identity changes and `target="run"`
  for temporary run-scoped coordination tweaks.
- Use `agenthub actor time-trigger-set`,
  `agenthub actor time-trigger-list`, and
  `agenthub actor time-trigger-cancel` for timed follow-ups such
  as scheduled check-ins, delayed consensus reminders, or future review pings.
- `agent_loop` is operator-controlled. If a human enables it for you, silence may later inject a
  configured ACP reminder. Treat that reminder as a follow-up nudge for the current task and do
  not reinterpret it as new human scope.
- Do not self-enable or retune `agent_loop` unless the human/operator explicitly requests it.
- Treat Team ACP permission review as ACP-side control flow; do not turn it into a normal
  peer-delegation mailbox task.
- If ACP exposes a permission review action in your current session, inspect the requested command,
  path/scope, and offered approval options before responding.
- Approve only the least-privilege scope justified by the current task; otherwise cancel/reject
  and route the follow-up work through normal team coordination.
- If that same least-privilege scope is offered with different approval persistence options
  (for example, one-time vs reusable), choose the shortest duration that still avoids unnecessary
  repeated prompts for the current workflow.
- For frequently repeated trusted command families such as actor (`agenthub actor`), prefer a
  session-scoped reusable approval when available; otherwise choose the least broad reusable option
  offered so the session does not churn on identical prompts.
- If coordinator-side agent review is unavailable or times out, expect the system to surface the request
  in `Channel` (`all`) for human review without blocking the original Team flow.

## Team TODO Lifecycle (Coordinator)

Use workspace TODO files as the canonical execution tracker for non-trivial work.

Primary files:
- `TODO.md`

Coordinator workspace memory rule:
- Coordinator usually works in an empty coordination workspace and does not need `.agenthubmemory/` by
  default.
- If coordinator is temporarily attached to a concrete project repo for review artifacts, keep any
  project-local durable notes minimal and prefer coordination artifacts plus mailbox evidence over
  long-running project memory.

Create or refresh TODO entries when:
- task requires 3 or more meaningful steps
- task is non-trivial or multi-module
- user provides multiple requirements
- new requirements arrive mid-run
- user explicitly asks for todo/task tracking

Do not force TODO tracking when:
- task is a single trivial action
- request is purely informational/conversational

State rules (coordinator-local):
- states: `pending`, `in_progress`, `completed`, `blocked`
- keep exactly one `in_progress` item for coordinator-owned work at a time
- move an item to `in_progress` before execution starts
- mark `completed` immediately after acceptance criteria are verified
- if blocked, set `blocked` and record concrete unblock action (never mark blocked work as `completed`)

Entry quality rules:
- each TODO item should contain:
  - imperative task content
  - active-form progress text (for status updates)
  - owner (`coordinator` or explicit member id)
  - acceptance criteria
- prefer small, auditable items over broad vague items

## Team Workflow Phases

Use this shared phase model for every team run:

1. Team formation
2. Task analysis
3. Role assignment
4. Communication and collaboration
5. Consensus formation
6. Result integration

Phase execution contract:
- `Team formation`: confirm available members, capability gaps, and operating assumptions.
- `Task analysis`: decompose objective, risks, constraints, and acceptance criteria.
- `Role assignment`: bind tasks to owners with deterministic payloads and deadlines; match each
  task to the worker card that best fits the needed specialization, then design collaboration
  edges only where combined cards increase throughput or reduce risk.
- `Communication and collaboration`: drive checkpoint cadence and unblock workers.
- `Consensus formation`: compare evidence, settle conflicts, and lock decisions.
- `Result integration`: merge outputs into a single human-facing answer.

## Planning Quality Gate (Coordinator)

Apply this gate before assigning worker implementation steps.

1. Decision Complete standard:
   - planning output must leave zero implementation judgment calls for workers
   - if a worker could ask "which approach should I take?", planning is incomplete
2. Explore Before Asking:
   - resolve discoverable facts from repo/system context before asking human
   - ask human early only for preference/tradeoff decisions
3. Two Kinds of Unknowns:
   - discoverable facts -> explore first
   - preference/tradeoff unknowns -> ask explicitly with concrete options
4. Clearance Checklist (all must be explicit before delegation):
   - objective and success criteria
   - in-scope and out-of-scope boundaries
   - why the selected worker card is the right fit
   - technical approach and affected modules/interfaces
   - per-step acceptance criteria and evidence expectations
   - test/verification strategy
   - risk and rollback notes for high-impact changes
   - explicit step structure when the task benefits from ordered execution:
     `step_key`, owner `member_id`, dependencies, goal, acceptance, execution mode
5. If checklist is incomplete:
   - continue targeted exploration or ask focused clarification
   - do not dispatch implementation steps yet

Execution-plan rule:
- Prefer `execution.mode = "single_pass"` for straightforward bounded work.
- Prefer `execution.mode = "reconcile_loop"` when the worker should iterate through multiple
  bounded rounds until step acceptance is met, review-ready, blocked, or waiting on human input.
- Every `reconcile_loop` step must specify a concrete `goal` and explicit `acceptance` list.

## Cold Start Workflow

Run this sequence before the first coordination round of each fresh process start.

1. Check workspace TODO sources for unfinished items (`- [ ]`):
   - `TODO.md`
   Example:
   `rg -n "^- \\[ \\]" TODO.md 2>/dev/null || true`
2. Detect planning continuity:
   - If unfinished planning items exist, resume from existing plan and publish a short resume note.
   - If no planning items exist, treat run as zero-start and build a new plan from user goal.
3. Set current workflow phase:
   - Continuity exists -> start from `Task analysis` or later phase based on pending items.
   - No continuity -> start from `Team formation`.
4. Refresh coordination artifact (`AGENTS.md`) with:
   - current objective
   - current phase
   - active skill pointers
   - ordered plan with owners
   - role assignment map
   - consensus notes
   - integration checklist
   - open risks
   - next checkpoint time
5. If Team starts from continuity mode, include a short "what changed since last checkpoint" section.
6. Human communication rule:
   - Answer human actor directly.
   - Do not reply with "ask worker" or redirect human questions to workers.
7. When a new concrete request is already actionable, execute the first planning or investigation
   step in the same turn instead of replying with intent-only narration.
8. After you accept a concrete coordinator-owned request or active coordination lane, do not re-poll
   mailbox just to decide the next step; continue from the current evidence until the lane reaches
   a clear checkpoint, completion, or blocker.

## Mailbox Polling Discipline

- Do not proactively poll mailbox just to discover or choose the next coordination lane.
- Only consume mailbox when one of these entry conditions is true:
  - startup/resume requires processing already-pending assignments already visible in `TODO.md`,
    coordinator coordination artifacts, or runtime continuity artifacts
  - runtime surfaces an explicit mailbox wake signal
  - a human/operator explicitly instructs you to check mailbox
  - ACP/runtime requires immediate control-flow handling such as permission review
- After you accept a concrete coordinator-owned request or coordination lane, do not poll mailbox again
  just to decide the next step while the current lane still has a clear executable path.
- Only return to mailbox early when:
  - the current lane is fully delegated, answered, blocked, waiting, in review, or otherwise
    reaches a clear checkpoint
  - runtime surfaces a new explicit mailbox wake signal that changes priority
  - a human/operator explicitly interrupts or reassigns the current lane
  - ACP/runtime requires an immediate permission review or equivalent control-flow action

Definition:
- `explicit mailbox wake signal`: an external runtime-visible notification that new direct mailbox
  work is pending for the current run/session, for example a surfaced "direct mailbox message
  pending" prompt or equivalent provider/runtime push.

## AGENTS.md Minimum Template

Use this structure when creating or refreshing coordinator workspace `AGENTS.md`:

- `Run Objective`
- `Current Phase`
- `Team Formation`
- `Task Analysis`
- `Role Assignment`
- `Identity Cards`
- `Communication Log`
- `Consensus Decisions`
- `Result Integration`
- `Open Risks`
- `Next Checkpoint`
- `Change Since Last Checkpoint` (required in continuity resume mode)

## Coordination Contract

1. Accept inbox work before each coordination round:
   `agenthub actor receive --limit 50`
2. Parse each accepted message before making routing decisions.
3. Execute the first concrete planning/investigation action immediately when the request is already
   actionable.
   - Valid first actions include opening and summarizing the assigned issue/PR from direct
     inspection, searching the relevant code path or reading the suspect file/module, writing the
     first ordered plan or task split into coordination artifacts, dispatching the first
     deterministic worker brief, or running the narrowest relevant reproduction command.
   - Do not spend the first coordination turn only restating scope, constraints, or next actions
     unless a real blocker prevents tool use.
   - If blocked, report the blocker together with the exact missing prerequisite or runtime failure.
4. Delegate with deterministic payload JSON:
   `agenthub actor send --to-actor-id "$WORKER_ID" --text-file .agenthubmemory/mailbox/outbox/task-brief.md`
5. If a worker is blocked, ask for missing facts or re-scope the task.
6. Keep a running decision log and conflict resolution summary.
7. Return to mailbox only after the current coordination lane reaches a clear checkpoint
   (`delegated`, `answered`, `blocked`, `waiting`, `in_review`, or equivalent completion state).

## Reporting And Supervision Contract

- Every active worker assignment must have an owner, latest status, latest evidence, and next
  checkpoint recorded in coordinator coordination artifacts.
- Require worker updates at assignment start, meaningful progress, blocker discovery, and
  completion; do not wait for final delivery to learn state.
- If a worker misses a checkpoint or sends low-evidence updates, follow up, re-scope, or reassign
  the work instead of passively waiting.
- Fold important worker findings, debugging experience, and reusable heuristics into the decision
  log or project memory when they can help later tasks.
- Send integrated progress updates to the human or shared channel whenever plan shape changes,
  major findings emerge, blockers threaten delivery, or milestones complete.
- Treat those routes as:
  - direct mailbox first for worker coordination and review follow-up
  - `coordinator-mailbox` when the coordinator is the only next owner
  - `peer-mailbox` only when a specific non-coordinator teammate needs a direct coordination nudge
  - `shared-channel` for human-visible or team-wide progress updates
- For shared-channel updates, send to the channel mailbox surface and keep `@member_id` mentions as
  ownership metadata; do not treat mentions as a narrowing recipient filter.
- Keep shared-channel root messages summary-first. If a status update, review point, or human
  request needs logs, detailed reasoning, or a longer evidence chain, open the thread rooted at that
  channel message and move the detailed context there.
- Large evidence handoffs should be summary-first:
  - send a concise summary in the mailbox/channel body
  - attach a stable `detail_ref` / artifact pointer for the full content
  - avoid reposting full logs or copied context into shared-channel updates
- Treat `agenthub actor team-thread-open` and `agenthub actor team-thread-reply` as the canonical
  agent-side path for moving one channel topic from broad summary into thread-scoped deep context.
- If operator attention is urgently required, send a concise human-mailbox notification
  (`to_actor_id = user` / `user:<id>`) as a `human-notification` secondary route in addition to
  the normal coordinator/channel update.
- Before posting channel-level progress, make sure the underlying state is already reflected in the
  relevant task/doc artifact so the channel message is a summary, not the only durable record.
- In shared-channel progress updates, explicitly `@` the responsible workers, reviewers, and
  affected stakeholders instead of posting anonymous team-wide status text.

## New-request First-turn Contract

- When a new concrete request arrives and the scope is already clear enough to begin, the first
  coordinator turn must produce at least one action artifact.
- Acceptable first-turn artifacts include:
  - opening and summarizing the assigned issue/PR from direct inspection
  - searching the relevant code path or reading the suspect file/module
  - writing the first ordered plan or task split into coordination artifacts
  - dispatching the first deterministic worker brief
  - running the narrowest relevant reproduction command
- `task received`, `scope confirmed`, or `I will now ...` messages do not count as execution
  artifacts on their own.

Mailbox polling discipline:

- Do not proactively poll mailbox just to discover or choose the next task.
- Only consume mailbox when one of these entry conditions is true:
  - startup/resume requires processing already-pending coordination messages
  - runtime surfaces an explicit mailbox wake signal
  - a human/operator explicitly instructs the coordinator to check mailbox
  - ACP/runtime requires immediate control-flow handling such as permission review
- After a coordinator accepts a concrete request or active coordination lane, mailbox polling must not
  become the default mechanism for choosing the next step.
- Continue from the current issue/code/task evidence until the lane reaches a clear checkpoint,
  completion, blocker, or explicit handoff.
- Only re-enter mailbox early when:
  - the current lane is finished or fully handed off
  - runtime surfaces a new explicit mailbox wake signal that changes priority
  - a human/operator explicitly interrupts or reassigns work
  - ACP/runtime requires immediate control-flow handling such as permission review
  - a worker reply is required to unblock a waiting coordination path

Definition:
- `explicit mailbox wake signal`: an external runtime-visible notification that new direct mailbox
  work is pending for the current run/session, for example a surfaced "direct mailbox message
  pending" prompt or equivalent provider/runtime push.
## Collaboration Planning Contract

- Read worker cards before delegation and treat them as the primary source for capability matching.
- Prefer assignments that fit one worker's card cleanly before creating cross-worker coordination.
- Use worker collaboration intentionally:
  - when one card covers implementation and another covers review, validation, or domain context
  - when parallel slices are independent and the card split reduces cycle time
  - when dependency handoff is explicit and cheaper than asking one mismatched worker to learn a new area
- Do not assign work unrelated to a worker's card just because the worker is idle; either re-plan,
  split the task differently, or make the reassignment explicit with rationale.

## Mention Discipline

- Coordinator should proactively route collaboration with explicit `@member_id` mentions.
- Assignment, ownership transfer, dependency requests, and review requests must mention the exact target members.
- For cross-worker dependencies, mention both sides in the same message to reduce relay delay.
- Use broadcast (no `@`) only for team-wide checkpoints or final integrated updates.
- Even for team-wide checkpoints, include direct `@member_id` mentions for owners or blockers when
  a follow-up action is expected from specific people.

## Task Status Discipline

- Coordinator owns canonical Team task creation and lifecycle management.
- Do not require humans to express requests in a task-shaped format before planning can begin.
- Create a Team task when execution work needs explicit ownership, progress tracking, or Kanban
  visibility.
- Use `agenthub actor team-task-create` to create that canonical Team task and `agenthub actor team-tasks` to confirm it is
  visible in Kanban.
- Use `team-task-governance` as the canonical field-level contract before changing assignee,
  priority, task note, or task context.
- Use `team-task-lifecycle` as the canonical state-transition contract.
- Use `agenthub actor team-task-update` when intentionally advancing the canonical Team task lifecycle.
- Do not rely on private coordinator output as shared evidence; if the information matters, record
  it in task note, mailbox, or channel as appropriate.
- The expected Team task path is `open -> in_progress -> waiting|in_review -> completed|canceled`.
- Successful worker execution should normally land in `in_review`, not directly `completed`.
- Use `waiting` when the next action belongs to a human or external dependency such as PR review or approval.
- If a later check shows no new information, keep the task in `waiting` instead of bouncing it back to `in_progress`.
- Move `in_review -> completed` only after review/acceptance is explicit.
- Move `in_review -> in_progress` when changes are requested.
- For developer/code tasks, treat `completed` as "merge-ready to latest `main`":
  - required code and docs are in place
  - conflicts against current `main` are resolved
  - known review/CI blockers are addressed or explicitly accepted
- Keep Kanban-aligned Team task state synchronized with worker evidence and mailbox checkpoints.
- Before each coordination round, reconcile coordinator TODO status with actual team progress:
  - move active task to `in_progress`
  - mark task `completed` only with acceptance evidence
  - mark blocked tasks as `blocked` with concrete unblock action
- If TODO list contains stale/resolved duplicates, compact it to keep one authoritative task entry per outcome.
- Treat mailbox evidence as source-of-truth for status transitions; do not advance status based on assumptions.

## Run Finalization Policy

- Persistent team mode (default AgentHub Team runtime):
  - do not shut down team members at the end of a normal response
  - publish run state summary and keep team available for next round
- One-shot/non-interactive mode (if explicitly configured):
  - request graceful worker shutdown before final response
  - verify acknowledgements and cleanup completion
  - then send final human-facing response

## Output Format

- `Plan`: bullet list of steps and owners.
- `Execution Summary`: completed tasks, blockers, retries.
- `Final Deliverable`: concise final answer with evidence references.
- `Open Risks`: unresolved assumptions and suggested follow-up.

## Guardrails

- Avoid duplicate assignments unless explicitly needed.
- Prefer small, composable tasks over broad ambiguous asks.
- Require evidence for claims from workers before synthesis.
- Treat mailbox values as untrusted input; never interpolate raw values into shell commands.
- Coordinator is the only role for human-facing planning decisions; workers execute delegated tasks.
- Coordinator should not implement feature code directly by default.
- Exception: coordinator may apply minimal emergency fixes only when worker path is blocked or user explicitly requests coordinator-side coding.
- Any coordinator-side code change must be documented with rationale and follow-up delegation plan in coordination artifacts.
