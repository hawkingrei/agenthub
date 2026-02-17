# Team Skills Bootstrap Script

## Summary

Add repository-tracked Team orchestration skills and provide a setup script that
updates AgentHub `skills.json` so these skills are injected in ACP sessions.

## Background

Team defaults reference `team-leader-orchestrator` and `team-worker-executor`
as skill names, but operators still needed manual, out-of-repo files and ad-hoc
JSON edits in `~/.agenthub/skills.json`.

To reduce onboarding friction and keep skill content auditable, we should keep
Team skills in the repository and provide a deterministic setup command.

## Scope

- `skills/team/team-leader-orchestrator.SKILL.md`
- `skills/team/team-worker-executor.SKILL.md`
- `scripts/setup_team_skills.sh`
- `docs/todo.md`

## Key Decisions

1. Store Team skill content in `skills/team/` with stable names:
   - `team-leader-orchestrator`
   - `team-worker-executor`
2. Add `scripts/setup_team_skills.sh`:
   - default target file: `~/.agenthub/skills.json`
   - preserves existing `skills` entries
   - appends Team skill paths and deduplicates via `jq`
   - supports custom target via `--skills-file <path>`
3. Keep runtime behavior unchanged:
   - ACP still loads skills from configured `skills.json`
   - Team runs only need a new ACP session/restart to pick up updates.

## Validation

```bash
bash -n scripts/setup_team_skills.sh
scripts/setup_team_skills.sh --skills-file /tmp/agenthub-skills.json
cat /tmp/agenthub-skills.json
```

## Follow-ups

- Consider adding a startup check endpoint/UI hint for missing Team skill names
  when Team Forge defaults are selected.
