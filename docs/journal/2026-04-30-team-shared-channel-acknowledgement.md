# Team Shared-Channel Acknowledgement

## Scope

Refine Team role prompts so human-authored shared-channel messages do not disappear into silent
background execution when a worker or leader is clearly engaged with the request.

## Decision

- Keep direct mailbox as the default execution path.
- Preserve shared-channel as the human-visible coordination surface.
- When a shared-channel message is relevant to an active worker, or is neutral-but-actionable shared
  context, require a short visible acknowledgement before deeper execution continues.
- Acceptable acknowledgement forms:
  - ownership
  - immediate plan
  - current progress

## Prompt Update

- Worker prompt now tells workers to reply briefly in shared-channel before continuing when they are
  the likely owner or a clearly relevant participant.
- Leader prompt now tells leader to ensure the team emits a quick visible acknowledgement instead of
  letting human channel input sit silently while coordination continues off-screen.

## Validation

- Prompt text review in:
  - `crates/agenthub-team-prompts/prompts/default_team_worker_prompt.txt`
  - `crates/agenthub-team-prompts/prompts/default_team_leader_prompt.txt`
- Collaboration contract update in:
  - `docs/features/teams-collaboration-playbook.md`
