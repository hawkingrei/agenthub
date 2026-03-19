# Team Runtime AGENTS Template (Single Template)

Use this single template for all Team members.
Keep runtime `AGENTS.md` minimal and role-scoped to control context size.

## Agent Profile

- member_id:
- role: `leader|worker`
- current phase: `team formation|task analysis|role assignment|communication and collaboration|consensus formation|result integration`
- transition condition:

## Objective

- current objective:
- success criteria:

## Active Assignment

- assignment summary:
- acceptance criteria:
- deadline:

## Active Skills (Load Only These)

- shared baseline (always):
  - `team-agents-index`
  - `team-actor-mailbox`
- role core (pick one):
  - leader: `team-leader-orchestrator`
  - worker: `team-worker-executor`
- optional (load only when needed):
  - `team-deliberation-rules`

## Shared Contracts

- Shared routing, mention, and human-facing reply rules live in `skills/team/AGENTS.md`.
- Leader-specific execution policy lives in `team-leader-orchestrator`.
- Worker-specific execution policy lives in `team-worker-executor`.
- Mailbox transport and payload details live in `team-actor-mailbox`.
- Self-profile updates use `profile_patch_proposal`.
- Timed self-reminders use `agent_time_trigger_set/list/cancel`.
- Load `team-deliberation-rules` only when you need option comparison or consensus work.

## TODO And Context Pointers

- `TODO.md`
- `.agenthubmemory/TODO.md` (worker/project workspace memory ledger, when applicable)
- `.agenthubmemory/journal/` (worker/project work log)
- `.agenthubmemory/note/` (worker/project durable lessons / heuristics)
- `.cache/context/` runtime continuity artifacts (not a durable TODO source)
- latest evidence/log paths:

## Progress Log

- status: `pending|in_progress|completed|blocked`
- latest update:
- next checkpoint:
