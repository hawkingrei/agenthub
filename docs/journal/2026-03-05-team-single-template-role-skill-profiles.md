# 2026-03-05 Team Single AGENTS Template And Role Skill Profiles

## Context

Team skill bootstrap still referenced separate leader/worker AGENTS templates.
That duplicated routing structure and encouraged loading broader context than needed.

## Changes

1. Switched to one runtime template:
   - `skills/team/TEAM_AGENTS.md`
   - one template for all members with explicit `role` and `Active Skills` sections.

2. Updated AGENTS index skills to apply role-specific profiles from one template:
   - `team-agents-index.SKILL.md`
   - `team-leader-agents-index.SKILL.md`
   - `team-worker-agents-index.SKILL.md`

3. Updated role execution skills to bootstrap from unified template:
   - `team-leader-orchestrator.SKILL.md`
   - `team-worker-executor.SKILL.md`

4. Updated Team policy docs and feature docs:
   - root `AGENTS.md`
   - `docs/features/agents-teams.md`
   - `docs/features/teams-collaboration-playbook.md`

5. Removed obsolete role-specific template files:
   - `skills/team/TEAM_LEADER_AGENTS.md`
   - `skills/team/TEAM_WORKER_AGENTS.md`

6. Updated Bazel export list to remove obsolete template references:
   - `BUILD.bazel`
   - keep `skills/team/TEAM_AGENTS.md` as the only runtime template export.

## Result

Team runtime now uses a single AGENTS template while keeping role-specific skill loading.
This reduces prompt/context size by default and keeps startup routing deterministic.
