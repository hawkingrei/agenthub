# Team Deliberation Rules Skill

## Background

Team collaboration already had orchestration and execution role skills, but lacked a shared
deliberation protocol for decision hygiene (assumptions, evidence, conflict resolution).

## Scope

- `skills/team/team-deliberation-rules.SKILL.md`
- `web/src/pages/team_page.tsx`
- `src/api/teams.rs`
- `scripts/setup_team_skills.sh`
- `docs/features/2026-02-17-team-skills-bootstrap-script.md`
- `docs/todo.md`

## Key Decisions

1. Add a dedicated Team skill: `team-deliberation-rules`.
2. Include this skill in Team default leader and worker skill sets.
3. Include this skill in Team bootstrap script output (`setup_team_skills.sh`) so
   `~/.agenthub/skills.json` can receive it with the existing team skills.

## Validation

Executed:

```bash
npm --prefix web run test -- src/pages/team_page.runs.test.ts
npm --prefix web run build
cargo test teams_api_ -- --nocapture
bash -n scripts/setup_team_skills.sh
scripts/setup_team_skills.sh --skills-file /tmp/agenthub-skills-deliberation-test.json
jq '.skills' /tmp/agenthub-skills-deliberation-test.json
```

Observed:

- Team page tests passed (`25 passed`).
- Team API tests passed (`9 passed`).
- Web production build succeeded.
- Team skills bootstrap preserved existing entries and appended
  `team-leader-orchestrator`, `team-worker-executor`, and
  `team-deliberation-rules` without dropping prior skills.
