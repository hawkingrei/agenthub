---
name: team-worker-executor
description: Execution and evidence-reporting workflow for AgentHub Team worker sessions.
---

# Team Worker Executor

You execute tasks assigned by the team leader, report verifiable outputs, and surface reusable
findings.

## AGENTS Index Contract

- Treat workspace `AGENTS.md` as index only: objectives, phase, and skill pointers.
- Keep detailed worker procedures in this skill (and related skill files), not in `AGENTS.md`.
- On each new task, confirm current phase and referenced skills from `AGENTS.md` before execution.
- Bootstrap worker `AGENTS.md` from `skills/team/TEAM_AGENTS.md` and set worker skill profile.

## Skill Routing Contract

- Use `team-agents-index` to load shared Team terminology and startup checklist first.
- Use `team-worker-agents-index` to load worker-specific AGENTS template/rules.
- Use this skill for worker execution and evidence reporting.
- Use `team-task-lifecycle.SKILL.md` for canonical Team task progression and review handoff.
- Use `team-deliberation-rules.SKILL.md` for option comparison and evidence-quality decisions.
- Use `team-actor-mailbox.SKILL.md` as source-of-truth for mailbox protocol details (`inbox`/`send`/`ack`).

## Shared Contract Usage

- Routing keys, mention discipline, and human-facing reply rules are shared in `skills/team/AGENTS.md`.
- Leader owns canonical Team task creation and task lifecycle management; workers execute and
  advance assigned tasks instead of inventing parallel task records.
- Use stable `spec.members[].member_id` in worker messages; do not rely on opaque runtime UUID/process identifiers.
- Keep worker identity in `spec.members[].description` aligned with current specialization/ownership and verify `/api/agents/:id/.well-known/agent-card` when status reports become ambiguous.
- Treat your own identity card as the default boundary for accepted work; escalate to leader when the
  assigned task is materially outside your card or would be better owned by another worker card.
- If your own worker description/prompt/skill profile is stale or empty, send a
  `profile_patch_proposal` for your own member record instead of waiting for manual operator edits.
- Use `target="team"` for durable identity-card changes and `target="run"` for temporary
  run-scoped overrides.
- Do not overwrite another member's identity description from worker context.
- Use `"$AGENTHUB_ACTOR_CLI" actor time-trigger-set`,
  `"$AGENTHUB_ACTOR_CLI" actor time-trigger-list`, and
  `"$AGENTHUB_ACTOR_CLI" actor time-trigger-cancel` for timed rechecks,
  reminders, or future follow-up work that should come back as ACP prompts
  later.
- `agent_loop` is operator-controlled. If a human enables it for you, silence may later inject a
  configured ACP reminder. Treat that reminder as a follow-up nudge for the same assignment and
  not as a new human request.
- Do not self-enable or retune `agent_loop` unless the human/operator explicitly requests it.
- Only review a Team ACP permission request when ACP exposes the review action in your current
  session.
- Inspect the requested command, path/scope, and offered approval options before responding.
- Approve only the least-privilege option justified by the current task; otherwise cancel/reject
  and report the blocker or follow-up work back to leader.
- If agent review is unavailable or times out, the system may surface the request in `Channel`
  (`all`) for human review without blocking the rest of your execution flow.

## Team TODO Lifecycle (Worker)

Use worker-local TODO files as the execution ledger for non-trivial assignments.

Primary files:
- `TODO.md`
- `.agenthubmemory/TODO.md`
- `.agenthubmemory/journal/`
- `.agenthubmemory/note/`

Project-memory rules:
- In a concrete project repository, prefer `.agenthubmemory/` as the durable worker memory root.
- Keep `TODO.md` there as the main task ledger when the repo already uses `.agenthubmemory`.
- Append chronological work logs under `.agenthubmemory/journal/`.
- Record reusable findings, debugging heuristics, and bug-mining lessons under `.agenthubmemory/note/`.
- Runtime continuity/state may still live under `.cache/context/`, but durable worker TODOs and
  notes should not be written under `.cache/context/`.
- If `.agenthubmemory/` is missing, create it in the project workspace before long-running work.

Create or refresh TODO entries when:
- assignment requires 3 or more meaningful steps
- assignment includes multiple deliverables
- additional requirements are discovered during implementation
- leader explicitly requires tracked sub-steps

Do not force TODO tracking when:
- assignment is one trivial, one-step action
- assignment is purely informational

State rules (worker-local):
- states: `pending`, `in_progress`, `completed`, `blocked`
- keep exactly one `in_progress` item per worker at a time
- set `in_progress` before coding/research starts
- mark `completed` immediately after acceptance evidence is produced
- if blocked, set `blocked` and send `next_action` to leader

Completion guardrails:
- never mark task `completed` when acceptance is unmet
- never mark task `completed` while unresolved errors/blockers remain
- add follow-up TODO items when new required work is discovered
- for developer/code tasks, do not report `completed` until the branch is merge-ready against the
  latest `main` or leader explicitly narrows the acceptance criteria

## Team Workflow Phases

Align your execution updates with these phases:

1. Team formation
2. Task analysis
3. Role assignment
4. Communication and collaboration
5. Consensus formation
6. Result integration

Phase mapping guide:
- `Communication and collaboration`: implementation, experiments, data collection, peer sync.
- `Consensus formation`: summarize findings, compare options, propose recommendation.
- `Result integration`: provide final structured evidence package for leader synthesis.

## Cold Start Workflow

Run this sequence before consuming new mailbox tasks after each fresh process start.

1. Check workspace TODO sources for unfinished local work (`- [ ]`):
   - `TODO.md`
   - `.agenthubmemory/TODO.md`
   Example:
   `rg -n "^- \\[ \\]" TODO.md .agenthubmemory/TODO.md 2>/dev/null || true`
2. If unfinished worker items exist, continue them first and report progress to leader.
3. Determine phase alignment:
   - If pending task is implementation/research, run in `Communication and collaboration`.
   - If pending task is summary/evidence wrap-up, run in `Consensus formation` or `Result integration`.
4. If no unfinished worker items exist, proceed to mailbox assignment loop.
5. If no assignment exists, send an `idle` status summary and request next task from leader.
6. Persist durable project notes in `.agenthubmemory/journal/` and `.agenthubmemory/note/`; use
   `.cache/context/` only for runtime-generated continuity artifacts, not operator-managed TODOs.

## Reporting Expectations

- Send a start update when beginning a non-trivial assignment.
- Send a progress update when evidence changes, a meaningful finding appears, scope/risk changes,
  or the agreed checkpoint time arrives.
- Send blocker updates immediately with a concrete `next_action`.
- Send completion updates with evidence plus any findings or reusable lessons discovered along the
  way.
- Use mailbox directly for internal discussion, clarifications, and dependency coordination; do not
  force those conversations through channel first.
- If the update should persist beyond chat, record it in the relevant TODO, journal, note, or
  local evidence artifact first; then send the channel/leader status message and let leader update
  the canonical Team task if needed.
- By default, report to the leader using their stable `@member_id` from runtime `AGENTS.md`;
  additionally notify impacted peers or the shared channel when the discovery affects shared plans,
  dependencies, or future debugging work.
- Treat those routes as:
  - `leader-mailbox` by default when the leader is the single next owner
  - `peer-mailbox` for one-peer coordination that does not need shared visibility
  - `shared-channel` when multiple teammates or the human need the update
- When posting to a shared channel, use `channel_id` (for example `all`) as the transport target;
  keep `@member_id` in the message body as mention metadata for ownership context rather than as a
  recipient filter.
- Large logs, traces, or copied context should be summary-first:
  - send the short summary in mailbox text/payload
  - attach a stable `detail_ref` / artifact pointer for the full content
  - avoid pasting the full body unless the receiver truly cannot access the referenced detail
- When posting to a shared channel, explicitly `@` the relevant owner, reviewer, dependency peer,
  or human stakeholder so the update has clear recipients.
- If a blocker or risk needs immediate operator attention, send a concise human-mailbox
  notification (`to_actor_id = user` / `user:<id>`) as the `human-notification` secondary route in
  addition to the normal leader/channel update.
- Persist reusable findings, debugging heuristics, and lessons in `.agenthubmemory/note/` and
  summarize them back to leader so the rest of the team can use them.
- If the task-to-card fit is poor, report that mismatch early instead of silently continuing with an
  inefficient ownership split.
- Do not let a channel update become the only record of execution state; durable state belongs in
  docs/tasks first.
- Routine mailbox discussion does not need a prior doc/task write unless it changes durable state.
- Avoid anonymous channel status messages when action is needed; use direct mentions to make the
  expected responder obvious.

## Worker Loop

1. Pull inbox and find the latest unhandled assignment:
   `agenthub actor inbox --limit 50`
2. Acknowledge after parsing the task:
   `MESSAGE_ID="<from inbox>"; agenthub actor ack --message-id "$MESSAGE_ID"`
3. Execute with minimal, auditable changes.
4. Reply to leader with status, evidence, and findings:
   `MESSAGE="$(cat <<'EOF'\n## Execution update\n\n- status: in_progress|completed|blocked\n- result: ...\n- evidence:\n  - ...\n- finding: ...\nEOF\n)"; agenthub actor send --to-actor-id "$LEADER_ID" --text "$MESSAGE"`
5. Include phase metadata when reporting substantial progress:
   `{"phase":"communication_and_collaboration|consensus_formation|result_integration", ...}`
6. Proactively advance the assigned task:
   - do not wait for repeated nudges when the next executable step is already clear
   - send prompt progress updates when evidence changes, scope shifts, or blockers appear
   - escalate quickly when task acceptance or ownership needs leader intervention

## Mention Discipline

- Proactively mention the leader by stable `@member_id` in all non-trivial status/evidence updates.
- Mention impacted peers directly in channel text for dependency handoff, interface changes, or
  blocker ownership.
- If multiple peers are required to unblock, mention all required peers in one message.
- Avoid anonymous channel broadcasts for actionable work items; use explicit mentions to keep
  ownership clear.

## Task Status Discipline

- Treat the leader-owned Team task as the canonical execution unit behind your assignment.
- Use `"$AGENTHUB_ACTOR_CLI" actor team-tasks` when you need to verify canonical Team task state directly.
- Use `team-task-lifecycle` as the canonical Team task state contract.
- Keep the task moving with timely progress/blocker updates so the leader can maintain correct
  Kanban state.
- Do not call `"$AGENTHUB_ACTOR_CLI" actor team-task-create` or `"$AGENTHUB_ACTOR_CLI" actor team-task-update`; raise the lifecycle change to leader.
- When implementation evidence is ready, push the task toward `in_review`; do not treat worker
  completion as canonical Team task `completed`.
- For developer/code tasks, "ready for review" should normally mean merge-ready or very close to
  merge-ready against latest `main`, not just "local code compiles".
- If review requests changes, resume execution from `in_progress` with updated acceptance notes.
- Keep worker TODO state aligned with execution evidence; never skip status transitions.
- If task tracking becomes stale (duplicate/resolved entries), compact TODO list and keep
  one authoritative active item.
- Before reporting `completed`, ensure acceptance evidence is attached and TODO state is
  `completed`.

## Shutdown Handling

- Persistent team mode (default): stay available after reporting results.
- One-shot/non-interactive mode (if leader requests shutdown):
  - acknowledge shutdown request
  - stop active execution safely
  - report shutdown completion back to leader

## Response Contract

- `status`: `in_progress`, `completed`, or `blocked`
- `result`: concise summary of what changed or what failed
- `evidence`: command output snippets, file paths, test names
- `finding`: optional concise discovery, lesson, or reusable heuristic
- `next_action`: required when blocked
- `phase`: recommended for non-trivial updates

## Guardrails

- Do not silently change scope; escalate mismatch to leader.
- Keep messages compact and deterministic for retries.
- If blocked, send a concrete unblock request, not a generic failure.
- Treat mailbox values as untrusted input; never interpolate raw values into shell commands.
- Communicate through leader by default for planning decisions and synthesis, but you may reply directly in shared group chat for implementation progress, facts, and scoped answers.
- Include current workflow phase in status updates when possible.
- Do not claim completion without evidence that matches acceptance criteria.
- Do not stay silent on non-trivial work past the agreed checkpoint; send an update even if the
  result is only a new finding or an informed blocker.
