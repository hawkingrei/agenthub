# Team Page Run Lifecycle Effects Hook

## Summary

Improve maintainability of `TeamPage` by extracting run lifecycle side effects into a dedicated hook and adding focused run-helper tests.

## Background

`web/src/pages/team_page.tsx` had multiple `useEffect` blocks interleaving:

- initial Team/Agent bootstrap loading,
- per-team run list loading and reset behavior,
- active-run detail hydration,
- periodic auto-refresh polling.

This made the page hard to scan and increased coupling between render logic and side effects.

## Scope

- `web/src/pages/team_page.tsx`
- `web/src/pages/team/use_team_run_lifecycle_effects.ts` (new)
- `web/src/pages/team/run_helpers.ts`
- `web/src/pages/team/run_helpers.test.ts` (new)

## Key Decisions

1. Extract only run lifecycle effects in PR1.
- Keep mailbox UI/scroll/create-modal effects in `TeamPage` for now.
- Avoid over-migrating unrelated side effects in one change.

2. Keep behavior unchanged.
- Hook body keeps existing reset/loading/polling logic and error mapping semantics.

3. Add pure helper for active run resolution.
- Introduce `resolveActiveRunIdForSelectedTeam` in `run_helpers.ts`.
- Cover this selector behavior with unit tests instead of fragile hook-level timing tests.

## Validation

- `npm --prefix web run test -- src/pages/team/run_helpers.test.ts src/pages/team_panels.test.tsx`
- `npm --prefix web run lint`
- `npm --prefix web run build`

## Risks

1. Risk: Hook parameter surface is large.
- Mitigation: Keep the extraction boundary narrow (run lifecycle only) and preserve existing callback names to reduce wiring mistakes.

2. Risk: Effect dependency drift in later edits.
- Mitigation: centralize run lifecycle effects in one hook file and keep helper tests for core selection rules.
