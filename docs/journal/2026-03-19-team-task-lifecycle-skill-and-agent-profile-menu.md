# Team Task Lifecycle Skill And Agent Profile Menu

## Summary

- added a dedicated `team-task-lifecycle` skill so Team prompts/skills no longer describe
  canonical task ownership and review transitions only as prose fragments
- aligned leader/worker Team prompts with the canonical task lifecycle, especially the
  `open -> in_progress -> in_review -> completed|canceled` contract
- added Team workspace agent-profile editing helpers and a lighter agent header menu so agent
  metadata can be inspected/edited without keeping a large header strip visible all the time

## Scope

- `skills/team/*.md`
- `crates/agenthub-team-prompts/src/lib.rs`
- `web/src/pages/team/member_helpers.ts`
- `web/src/pages/team/create_helpers.ts`
- `web/src/pages/team_page.tsx`

## Key Changes

1. Added `skills/team/team-task-lifecycle.SKILL.md`
   - defines canonical Team task states
   - defines leader/worker permissions for task state changes
   - explains how worker-local TODO state maps to canonical Team task state
2. Routed shared/team role skills to `team-task-lifecycle`
   - `AGENTS.md`
   - `TEAM_AGENTS.md`
   - `team-agents-index`
   - `team-leader-agents-index`
   - `team-worker-agents-index`
   - leader/worker role skills
3. Updated Team prompt templates and frontend prompt mirrors
   - explicit `in_review` contract
   - explicit `team-task-lifecycle` loading rule
4. Added editable Team member profile helpers
   - read Team member draft from spec
   - update Team member description/model/prompt/skills in spec without dropping runtime hints
5. Slimmed Team agent workspace header
   - moved heavy role/lifecycle/inbox/current-work details into an `Agent` menu
   - added `Edit profile` modal from the Team agent workspace

## Validation

- `cargo test -p agenthub-team-prompts`
- `cd web && npx vitest run src/pages/team/create_helpers.test.ts src/pages/team/member_helpers.test.ts`
- `cd web && npm run lint -- src/pages/team/create_helpers.ts src/pages/team/create_helpers.test.ts src/pages/team/member_helpers.ts src/pages/team/member_helpers.test.ts src/pages/team_page.tsx`
- `cd web && npm run build`
- `git -c core.fsmonitor=false diff --check`

## Follow-up

- frontend still carries a prompt mirror (`member_helpers.ts`) instead of consuming a canonical
  backend-exported prompt contract; keep reducing drift in future work
