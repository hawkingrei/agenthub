# Team Single-Node Skill Bootstrap Hardening

## Summary

Harden Team skill bootstrap for single-node usage by making
`scripts/setup_team_skills.sh` install Team skill files under
`~/.agenthub/worktrees/team-skills` by default, then writing those installed
paths into `~/.agenthub/skills.json`.

## Background

Team skill paths loaded from an arbitrary repository location can be skipped by
ACP runtime when they are outside configured `safe_paths`.

For single-node users, the default safe path already includes
`~/.agenthub/worktrees`, so writing skill files there is the lowest-friction
way to guarantee Team role skill loading without extra config edits.

## Scope

- `scripts/setup_team_skills.sh`
- `userdocs/docs/advanced/team-workbench.md`
- `docs/todo.md`

## Key Decisions

1. Keep `scripts/setup_team_skills.sh` as the canonical Team bootstrap entry.
2. Default behavior now copies Team skills into:
   - `~/.agenthub/worktrees/team-skills`
3. Keep an explicit compatibility switch:
   - `--use-repo-skill-paths` (no copy; writes repo paths directly)
4. Keep existing JSON merge contract:
   - preserve existing entries
   - support both string/object skill entries
   - dedupe by resolved skill path while preserving insertion order

## Validation

```bash
bash -n scripts/setup_team_skills.sh

tmp_dir="$(mktemp -d)"
scripts/setup_team_skills.sh \
  --skills-file "${tmp_dir}/skills.json" \
  --install-dir "${tmp_dir}/team-skills"
cat "${tmp_dir}/skills.json"
ls -la "${tmp_dir}/team-skills"

scripts/setup_team_skills.sh \
  --skills-file "${tmp_dir}/skills-repo.json" \
  --use-repo-skill-paths
cat "${tmp_dir}/skills-repo.json"
```

## Follow-ups

- Add a lightweight API/UI preflight check that warns when Team-required skills
  are missing from runtime-visible skill paths before launching Team runs.
