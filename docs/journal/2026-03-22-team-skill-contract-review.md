# Team Skill Contract Review

## Summary

- reviewed the full `skills/team/*` set after the recent reporting-guidance and actor/channel routing changes
- reduced ambiguous routing language in Team skills by introducing one shared vocabulary:
  - `leader-mailbox`
  - `peer-mailbox`
  - `shared-channel`
  - `human-notification`
- removed the old `leader+channel` / `both` style wording from the runtime template because
  channel delivery is already team-wide fan-out
- clarified that direct mailbox is single-target, while channel delivery is team-wide and `@member_id`
  remains metadata rather than a recipient filter
- reinforced that shared-channel text should stay concise and human-readable; phase/transport details
  belong in mailbox or durable artifacts

## Files Reviewed

- `skills/team/AGENTS.md`
- `skills/team/TEAM_AGENTS.md`
- `skills/team/team-agents-index.SKILL.md`
- `skills/team/team-deliberation-rules.SKILL.md`
- `skills/team/team-actor-mailbox.SKILL.md`
- `skills/team/team-leader-agents-index.SKILL.md`
- `skills/team/team-leader-orchestrator.SKILL.md`
- `skills/team/team-task-lifecycle.SKILL.md`
- `skills/team/team-worker-agents-index.SKILL.md`
- `skills/team/team-worker-executor.SKILL.md`

## Key Adjustments

### Shared Contract Consolidation

- moved route naming authority into `skills/team/AGENTS.md`
- made downstream skills reference that shared vocabulary instead of inventing their own mixed
  transport/intent labels

### Runtime Template Cleanup

- changed progress log fields from a loose `report target` example list to:
  - `primary route`
  - `secondary route`
- kept `human-notification` as an explicit optional secondary route for urgent operator escalation

### Mailbox Semantics

- documented direct mailbox as single-target only
- documented shared channel as the team-wide mailbox surface
- documented human mailbox as urgent operator-facing notification
- added an explicit single-peer mailbox example alongside shared-channel and human-notification

### Leader / Worker Guidance

- aligned both role skills to the same routing vocabulary
- kept leader as owner of integrated human/channel progress
- kept worker default route as leader mailbox, with shared channel only when team-wide visibility
  is actually needed

## Validation

- `git -c core.fsmonitor=false diff --check`
