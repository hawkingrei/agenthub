# Team Worker Group Chat Replies

## Context

Team prompts were still biasing workers toward routing all human-visible replies through leader. That was too strict for shared group chat, where workers should be able to answer with implementation progress, concrete facts, and scoped results directly.

## Decision

- Keep leader as the owner of planning decisions and final synthesis.
- Allow workers to reply directly in shared group chat.
- Limit direct worker replies to:
  - implementation progress
  - concrete facts/evidence
  - scoped answers within the assigned work
- Do not allow worker replies to override leader planning decisions or final integrated response.

## Updated Prompt Sources

- `AGENTS.md`
- `skills/team/AGENTS.md`
- `skills/team/TEAM_AGENTS.md`
- `skills/team/team-actor-mailbox.SKILL.md`
- `skills/team/team-deliberation-rules.SKILL.md`
- `skills/team/team-worker-executor.SKILL.md`
