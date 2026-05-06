# Team Adoption Move Deferred

## Summary

`Add Existing Agent` now keeps `Copy into Team` as the only executable adoption path while showing
`Move to Team` as explicitly deferred.

## Background

The Team adoption contract separates copying an existing agent from moving the original agent into
Team ownership. Copy is safe for the first rollout because it creates a new Team-owned member and
leaves the source agent unchanged. Move needs stricter runtime, ownership, and history guardrails
before it can be enabled.

## Scope

- Keep copy as the active Add Existing Agent action.
- Add visible move semantics explaining why transfer is not enabled yet.
- Render a disabled `Move to Team (later)` action so copy and move are not collapsed into one
  ambiguous import path.

## Key Decisions

- Do not add move behavior in this slice.
- Do not mutate source agents from the Add Existing Agent flow.
- Keep the disabled move action in the same modal as copy so operators see the product boundary at
  the decision point.

## Validation

```bash
npm --prefix web run test -- src/pages/team/team_management_modals.test.tsx
git diff --check
npm --prefix web run lint
npm --prefix web run build
```

## Follow-Ups

- Define stopped-only runtime and ownership-transfer guards before enabling `Move to Team`.
- Add browser-level coverage for copy-first adoption and the disabled move boundary.
