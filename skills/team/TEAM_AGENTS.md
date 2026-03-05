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

## Role Skill Profile

Leader minimal profile:

- default loaded:
  - `team-leader-orchestrator`
  - `team-actor-mailbox`
- load on demand:
  - `team-deliberation-rules` (only for option comparison/consensus disputes)

Worker minimal profile:

- default loaded:
  - `team-worker-executor`
  - `team-actor-mailbox`
- load on demand:
  - `team-deliberation-rules` (only for trade-off evaluation requested by leader)

## Routing Contract

- `@member_id` means directed recipients only.
- no `@` means broadcast.
- Prefer directed mentions for execution collaboration and blocker resolution.

## TODO And Context Pointers

- `TODO.md`
- `.cache/context/todo.md`
- latest evidence/log paths:

## Progress Log

- status: `pending|in_progress|completed|blocked`
- latest update:
- next checkpoint:
