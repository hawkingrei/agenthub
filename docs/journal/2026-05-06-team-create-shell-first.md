# Team Create Shell First

## Summary

Team creation now stops at the Team shell. It no longer opens the first coordinator-agent forge
modal immediately after `Create Team` succeeds. The mobile browser flow now also covers creating a
Team shell, copying the first existing agent as coordinator, and copying a later existing agent as
worker without a role switch.

## Background

The canonical Team create flow keeps `Create Team` mission-first and moves coordinator assignment
to the first explicit `Add Agent` action. The UI already made the add-agent role fixed
(`coordinator` for an empty Team, `worker` after a coordinator exists), but the create action still
opened the coordinator forge modal automatically.

## Scope

- Keep the `Create Team` action limited to creating an empty Team spec.
- Reset the create/forge draft state after creation.
- Leave the operator on the new Team detail page with a direct next-step hint.
- Preserve the existing `Add First Coordinator Agent` and `Copy Existing Agent` entry points as the
  places where first-agent coordinator semantics are shown.
- Cover the small-screen `Create Team -> Copy Existing Agent as coordinator -> Copy Existing Agent
  as worker` path.

## Key Decisions

- Do not auto-open the forge modal after creating a Team shell.
- Do not fabricate a draft coordinator member during Team creation.
- Keep role assignment inside the add-agent flow, where the selected Team state determines whether
  the fixed role is `coordinator` or `worker`.

## Validation

```bash
npm --prefix web run test -- src/pages/team/use_team_management_actions.test.tsx src/pages/team/team_management_modals.test.tsx
(cd web && PLAYWRIGHT_NO_WEBSERVER=1 PLAYWRIGHT_MOBILE_ONLY=1 npm exec -- playwright test --project chromium)
git diff --check
npm --prefix web run lint
npm --prefix web exec -- tsc -p web/tsconfig.json --noEmit
npm --prefix web run build
```

## Follow-Ups

- Keep future Team create changes aligned with the mission-first shell and fixed-role add-agent
  contract in `docs/features/team-create-flow.md`.
