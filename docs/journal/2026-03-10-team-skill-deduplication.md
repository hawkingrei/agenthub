# Team Skill Deduplication

## Goal

Reduce repeated routing and reply-contract text across `skills/team/*` without
changing Team behavior.

## Changes

- Marked `skills/team/AGENTS.md` as the canonical shared contract for:
  - routing keys
  - mention discipline
  - human-facing reply policy
  - startup checklist
- Shrunk `skills/team/TEAM_AGENTS.md` into a runtime template that references
  shared contracts instead of restating them.
- Replaced duplicated routing/identity sections in:
  - `team-leader-orchestrator.SKILL.md`
  - `team-worker-executor.SKILL.md`
  with short references back to the shared Team baseline.
- Reduced `team-actor-mailbox.SKILL.md` to mailbox transport details plus the
  minimal human-facing envelope note.
- Reduced `team-deliberation-rules.SKILL.md` to deliberation-specific guidance
  and referenced the shared Team contract for human-facing routing.

## Result

- Shared Team rules now have a single fact source in `skills/team/AGENTS.md`.
- Role and protocol skills keep only role-specific or transport-specific
  details.
- Future policy edits should touch fewer files and are less likely to drift.
