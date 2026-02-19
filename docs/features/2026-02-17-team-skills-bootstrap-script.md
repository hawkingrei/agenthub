# Team Skills Bootstrap Script

## Summary

Add repository-tracked Team orchestration skills and provide a setup script that
updates AgentHub `skills.json` so these skills are injected in ACP sessions.

## Background

Team defaults reference `team-leader-orchestrator`, `team-worker-executor`, and
`team-deliberation-rules`
as skill names, but operators still needed manual, out-of-repo files and ad-hoc
JSON edits in `~/.agenthub/skills.json`.

To reduce onboarding friction and keep skill content auditable, we should keep
Team skills in the repository and provide a deterministic setup command.

## Scope

- `skills/team/team-leader-orchestrator.SKILL.md`
- `skills/team/team-worker-executor.SKILL.md`
- `skills/team/team-deliberation-rules.SKILL.md`
- `scripts/setup_team_skills.sh`
- `docs/todo.md`

## Key Decisions

1. Store Team skill content in `skills/team/` with stable names:
   - `team-leader-orchestrator`
   - `team-worker-executor`
   - `team-deliberation-rules`
2. Add `scripts/setup_team_skills.sh`:
   - default target file: `~/.agenthub/skills.json`
   - preserves existing `skills` entries (both string and object entries)
   - appends Team skill paths and deduplicates while preserving order
   - supports custom target via `--skills-file <path>` and `--skills-file=<path>`
3. Keep runtime behavior unchanged:
   - ACP still loads skills from configured `skills.json`
   - Team runs only need a new ACP session/restart to pick up updates.
   - Skill files must be under ACP `safe_paths`; otherwise they are skipped.

## Validation

```bash
bash -n scripts/setup_team_skills.sh
scripts/setup_team_skills.sh --skills-file /tmp/agenthub-skills.json
cat /tmp/agenthub-skills.json
```

2026-02-18 local verification:

```bash
tmp_skills="/tmp/agenthub-team-skills-test.json"
cat > "$tmp_skills" <<'EOF'
{"skills":["/existing/skill-a.SKILL.md",{"path":"/Users/weizhenwang/devel/opensource/agenthub/skills/team/team-leader-orchestrator.SKILL.md"},"/existing/skill-b.SKILL.md"]}
EOF
scripts/setup_team_skills.sh --skills-file "$tmp_skills"
jq -c '.skills' "$tmp_skills"
```

Observed:

- existing entries remained intact;
- leader skill path was not duplicated when already present as object entry;
- worker and deliberation skill paths were appended.

## Follow-ups

- Consider adding a startup check endpoint/UI hint for missing Team skill names
  when Team Forge defaults are selected.
