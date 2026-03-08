# Team Human-Facing Reply Contract

## Context

Team mailbox skills were still encouraging agents to emit structured mailbox status fields in human-visible chat replies. In practice this leaked transport-oriented content such as `current_phase` or raw JSON wrappers into the shared thread UI.

## Decision

- Keep structured mailbox payloads for internal execution coordination only.
- Human-facing team conversation replies must contain final reply content only.
- Do not expose mailbox transport status, `current_phase`, or raw JSON envelope fields in visible chat text.
- If transport still needs a chat envelope, keep it transport-only and place the natural-language reply in `text`.

## Updated Prompt Sources

- `AGENTS.md`
- `skills/team/AGENTS.md`
- `skills/team/TEAM_AGENTS.md`
- `skills/team/team-actor-mailbox.SKILL.md`
- `skills/team/team-deliberation-rules.SKILL.md`

## Expected Outcome

- Leader and workers continue to exchange structured mailbox status/evidence internally.
- Human-visible team chat becomes natural-language only.
- Shared thread rendering no longer depends on exposing mailbox protocol details to users.
