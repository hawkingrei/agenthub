# Team Cold-Start Skill And UI Playbook

## Background

Team runs needed a clearer startup contract for both leader and worker roles:

- each agent should check local TODO continuity before new mailbox work
- leader should determine whether to resume an existing plan or start from zero
- leader should communicate planning decisions directly with the human actor

The previous role prompts and skill files did not enforce these startup expectations strongly enough, and `/teams` UI did not present the operating model clearly.

## Scope

- Expand Team role skill instructions (`leader`, `worker`, `deliberation`) with explicit cold-start workflow.
- Expand Team default role prompts with TODO-first startup policy and leader-human communication boundary.
- Reorganize `/teams` UI panels to surface operating model and cold-start playbook.

Out of scope:

- runtime filesystem scanning/enforcement of TODO files in backend
- introducing new Team role skill names or changing required skill IDs

## Key Decisions

1. Keep existing Team role skill IDs (`team-leader-orchestrator`, `team-worker-executor`, `team-deliberation-rules`) and extend their content.
2. Define cold-start TODO scan paths consistently:
   - `TODO.md`
   - `.cache/context/todo.md`
3. Make leader-human communication boundary explicit in both skill and default prompt text:
   - leader answers planning questions directly
   - worker reports through leader by default
4. Surface the same policy in UI:
   - Team Sidebar adds an "Operating Model" note
   - Team Overview adds a "Cold Start Playbook" card (leader startup + worker startup)

## Files Changed

- `skills/team/team-leader-orchestrator.SKILL.md`
- `skills/team/team-worker-executor.SKILL.md`
- `skills/team/team-deliberation-rules.SKILL.md`
- `web/src/pages/team/member_helpers.ts`
- `web/src/pages/team_overview_panel.tsx`
- `web/src/pages/team_sidebar.tsx`
- `web/src/pages/team_panels.test.tsx`
- `docs/todo.md`

## Validation

Executed during development:

- `npm --prefix web run test -- src/pages/team_panels.test.tsx`
- `npm --prefix web run test -- src/pages/team_page.runs.test.ts`
- `cargo test -p agenthub-acp build_team_role_skills_for_leader -- --nocapture`

## Risks And Follow-up

- This change is prompt/skill/UI guidance level; backend does not yet hard-enforce TODO file presence.
- Existing teams with custom persisted prompts may keep prior wording; role skill injection still provides startup policy at runtime.
