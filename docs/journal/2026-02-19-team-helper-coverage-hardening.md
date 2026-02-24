# Team Helper Coverage Hardening

## Summary

Add focused unit tests for Team helper/state modules to improve PR patch coverage and prevent regressions in parsing, reducer transitions, and list-upsert semantics.

## Background

PR #61 introduced substantial Team Workbench refactors and helper extraction. Codecov patch gating reported low direct coverage in:

- `web/src/pages/team/create_helpers.ts`
- `web/src/pages/team/state.ts`
- `web/src/pages/team/page_helpers.ts`

These modules contain branch-heavy pure logic and are best validated with deterministic unit tests instead of additional E2E-only coverage.

## Scope

- `web/src/pages/team/create_helpers.test.ts`
- `web/src/pages/team/state.test.ts`
- `web/src/pages/team/page_helpers.test.ts`
- `docs/todo.md`

## Key Decisions

1. Test pure helper contracts directly from extracted modules instead of through `team_page.tsx` re-exports.
2. Cover error and boundary paths explicitly:
   - JSON parsing and integer validation (`create_helpers`)
   - worktree/workdir error mapping (`create_helpers`)
   - reducer unknown-action no-op branches (`state`)
   - conversation seen-watermark edge branches (`state`)
3. Keep list merge assertions aligned with current `prepend` semantics in page helper upsert functions (existing list wins on duplicate ids).

## Validation

Executed (2026-02-19):

```bash
npm --prefix web run test -- src/pages/team/create_helpers.test.ts src/pages/team/state.test.ts src/pages/team/page_helpers.test.ts
npm --prefix web run test
npm --prefix web run test:coverage -- src/pages/team/create_helpers.test.ts src/pages/team/state.test.ts src/pages/team/page_helpers.test.ts
```

Coverage snapshot from generated `web/coverage/lcov.info` (target files):

- `src/pages/team/create_helpers.ts`: `73/75` lines
- `src/pages/team/state.ts`: `39/39` lines
- `src/pages/team/page_helpers.ts`: `21/21` lines
