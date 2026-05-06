# Team Copy Conflict Cleanup

## Summary

Copying an existing agent into a Team now best-effort deletes the newly created Team-owned copy when
the follow-up Team spec update fails with a conflict.

## Background

The adoption copy path creates a new Team-owned agent first, then appends that agent to the Team
spec. If the Team spec update races another edit and returns `409`, leaving the created copy around
violates the copy rollout's ownership boundary by producing an unattached Team-forged agent.

## Scope

- Apply the same conflict cleanup shape already used by the Team forge path.
- Keep source-agent copy semantics unchanged.
- Do not implement `move existing agent to Team`.

## Key Decisions

- Cleanup only runs after a copied agent was created and the Team spec update reports `409`.
- Cleanup remains best-effort so a delete failure does not hide the original conflict error.
- The copy modal stays open on conflict so the operator can retry after refreshing state.

## Validation

```bash
npm --prefix web run test -- src/pages/team/use_team_management_actions.test.tsx
git diff --check
npm --prefix web run lint
npm --prefix web exec -- tsc -p web/tsconfig.json --noEmit
npm --prefix web run build
```

## Follow-Ups

- Continue the Team adoption backlog with explicit workspace-content copy, memory/context seeding,
  and move-ownership guardrails as separate opt-in slices.
