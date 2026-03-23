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

Review follow-ups:

- `actor help` now reserves literal `help` for the explicit help subcommand position only;
  generic flag-value parsing only treats `--help` / `-h` as help flags so values like
  `--team-id help` continue to parse normally.
- worker execution examples now use `agenthub actor ack/send ...` consistently after the
  canonical `agenthub actor inbox` entrypoint.
- internal permission-review control now rejects requests that set both `option_id` and
  `outcome`, so the server-side contract matches the CLI's mutual-exclusion rule.
