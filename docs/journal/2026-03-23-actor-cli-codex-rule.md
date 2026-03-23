Added a user-facing Team Workbench note for local Codex environments that run
the canonical actor CLI mailbox path.

Updated:

- `userdocs/docs/advanced/team-workbench.md`

New guidance:

- append `prefix_rule(pattern=["agenthub", "actor"], decision="allow")`
  to `~/.codex/rules/default.rules`
- prefer the short actor CLI workflow:
  - `agenthub actor inbox`
  - `agenthub actor ack --message-id <id>`
  - `agenthub actor send --to-actor-id <actor_id> --text "<markdown>"`

Rationale:

- Team runtime coordination is now CLI-first for mailbox work
- local Codex approval prompts should not interrupt repeated mailbox commands
- agent-facing reminders and runtime examples now prefer the shorter
  `agenthub actor ...` form instead of spelling out `"$AGENTHUB_ACTOR_CLI"`
