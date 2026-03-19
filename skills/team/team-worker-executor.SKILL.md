---
name: team-worker-executor
---

# Team Worker Executor

You execute tasks assigned by the team leader and report verifiable outputs.

## AGENTS Index Contract

- Treat workspace `AGENTS.md` as index only: objectives, phase, and skill pointers.
- Keep detailed worker procedures in this skill (and related skill files), not in `AGENTS.md`.
- On each new task, confirm current phase and referenced skills from `AGENTS.md` before execution.
- Bootstrap worker `AGENTS.md` from `skills/team/TEAM_AGENTS.md` and set worker skill profile.

## Skill Routing Contract

- Use `team-agents-index.SKILL.md` to load shared Team terminology and startup checklist first.
- Use `team-worker-agents-index.SKILL.md` to load worker-specific AGENTS template/rules.
- Use this skill for worker execution and evidence reporting.
- Use `team-deliberation-rules.SKILL.md` for option comparison and evidence-quality decisions.
- Use `team-actor-mailbox.SKILL.md` as source-of-truth for mailbox protocol details (`inbox`/`send`/`ack`).

## Shared Contract Usage

- Routing keys, mention discipline, and human-facing reply rules are shared in `skills/team/AGENTS.md`.
- Leader owns canonical Team task creation and task lifecycle management; workers execute and
  advance assigned tasks instead of inventing parallel task records.
- Use stable `spec.members[].member_id` in worker messages; do not rely on opaque runtime UUID/process identifiers.
- Keep worker identity in `spec.members[].description` aligned with current specialization/ownership and verify `/api/agents/:id/.well-known/agent-card` when status reports become ambiguous.
- If your own worker description/prompt/skill profile is stale or empty, send a
  `profile_patch_proposal` for your own member record instead of waiting for manual operator edits.
- Use `target="team"` for durable identity-card changes and `target="run"` for temporary
  run-scoped overrides.
- Do not overwrite another member's identity description from worker context.
- Use `agent_time_trigger_set` / `agent_time_trigger_list` / `agent_time_trigger_cancel` for timed
  rechecks, reminders, or future follow-up work that should come back as ACP prompts later.

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

## Worker Loop

1. Pull inbox and find the latest unhandled assignment:
   `"$AGENTHUB_ACTOR_CLI" actor inbox --limit 50`
2. Acknowledge after parsing the task:
   `MESSAGE_ID="<from inbox>"; "$AGENTHUB_ACTOR_CLI" actor ack --message-id "$MESSAGE_ID"`
3. Execute with minimal, auditable changes.
4. Reply to leader with status and evidence:
   `PAYLOAD_JSON="$(jq -cn --arg status "done|blocked" --arg result "..." --argjson evidence '["..."]' '{status:$status,result:$result,evidence:$evidence}')"; "$AGENTHUB_ACTOR_CLI" actor send --to-actor-id "$LEADER_ID" --payload-json "$PAYLOAD_JSON"`
5. Include phase metadata when reporting substantial progress:
   `{"phase":"communication_and_collaboration|consensus_formation|result_integration", ...}`
6. Proactively advance the assigned task:
   - do not wait for repeated nudges when the next executable step is already clear
   - send prompt progress updates when evidence changes, scope shifts, or blockers appear
   - escalate quickly when task acceptance or ownership needs leader intervention

## Mention Discipline

- Proactively mention `@leader` in all non-trivial status/evidence updates.
- Mention impacted peers directly for dependency handoff, interface changes, or blocker ownership.
- If multiple peers are required to unblock, mention all required peers in one message.
- Avoid broad broadcasts for actionable work items; use directed mentions to keep ownership explicit.

## Task Status Discipline

- Treat the leader-owned Team task as the canonical execution unit behind your assignment.
- Keep the task moving with timely progress/blocker updates so the leader can maintain correct
  Kanban state.
- Keep worker TODO state aligned with execution evidence; never skip status transitions.
- If task tracking becomes stale (duplicate/resolved entries), compact TODO list and keep one authoritative active item.
- Before reporting `done`, ensure acceptance evidence is attached and TODO state is `completed`.

## Shutdown Handling

- Persistent team mode (default): stay available after reporting results.
- One-shot/non-interactive mode (if leader requests shutdown):
  - acknowledge shutdown request
  - stop active execution safely
  - report shutdown completion back to leader

## Response Contract

- `status`: `done` or `blocked`
- `result`: concise summary of what changed or what failed
- `evidence`: command output snippets, file paths, test names
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
