# Team Adoption Move Deferred

## Summary

`Add Existing Agent` now keeps `Copy into Team` as the only executable adoption path while
documenting `Move to Team` as explicitly deferred.

## Background

The Team adoption contract separates copying an existing agent from moving the original agent into
Team ownership. Copy is safe for the first rollout because it creates a new Team-owned member and
leaves the source agent unchanged. Move needs stricter runtime, ownership, and history guardrails
before it can be enabled.

## Scope

- Keep copy as the active Add Existing Agent action.
- Add visible move semantics explaining why transfer is not enabled yet.
- Keep move as non-actionable explanatory copy instead of a default add-agent action.
- Make the default copy boundary explicit: copied configuration does not clone workspace contents,
  runtime history, active sessions, or workspace-local memory/context.

## Key Decisions

- Do not add move behavior in this slice.
- Do not mutate source agents from the Add Existing Agent flow.
- Keep move semantics in the same modal as copy so operators see the product boundary at the
  decision point, without presenting move as a selectable action.
- Keep workspace-content copy, memory/context seeding, and ownership transfer as separate opt-in
  follow-ups rather than implicit side effects of configuration copy.

## Validation

```bash
npm --prefix web run test -- src/pages/team/team_management_modals.test.tsx
git diff --check
npm --prefix web run lint
npm --prefix web exec -- tsc -p web/tsconfig.json --noEmit
npm --prefix web run build
```

## 2026-05-06 Browser Coverage Follow-Up

The Team setup E2E path now covers the copy-first adoption boundary:

- an empty Team can open `Copy Existing Agent` from the setup panel;
- `Move to Team (later)` is not shown as a default action;
- the modal explains that move semantics are not enabled yet;
- the modal explains that the default copy path is configuration-only;
- `Copy into Team` creates a new Team-owned coordinator member;
- the original source agent remains available in the mocked agent list.

Additional validation:

```bash
npm --prefix web run e2e -- tests/e2e/team_page_setup.e2e.ts --grep "team adoption copy keeps move disabled"
```

## Follow-Ups

- Define stopped-only runtime and ownership-transfer guards before enabling `Move to Team`.
- Separately evaluate explicit workspace-content copy and memory/context seeding before any
  non-configuration adoption mode is enabled.
