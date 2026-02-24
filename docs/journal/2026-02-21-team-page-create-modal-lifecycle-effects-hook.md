# Team Page Create-Modal Lifecycle Effects Hook

## Summary

Continue `TeamPage` maintainability refactor by extracting create-modal and runtime-default lifecycle side effects into a dedicated hook.

## Background

After run-lifecycle and mailbox/member-lifecycle extractions, `TeamPage` still kept create-flow-focused effects inline:

- runtime default worktree-root bootstrap by token,
- modal-open leader fallback selection,
- Escape-key close behavior with busy-state guard.

Keeping these effects inside the page component still increased local effect density and made create-flow behavior harder to audit.

## Scope

- `web/src/pages/team/use_team_create_modal_lifecycle_effects.ts` (new)
- `web/src/pages/team_page.tsx`

## Key Decisions

1. Keep extraction narrow.
- This PR only moves create-modal/runtime-default lifecycle effects.
- Member-runtime backfill and local ref-sync effects stay in `TeamPage` for later slices.

2. Preserve existing behavior.
- Runtime default root fetch still uses the same API path and fallback logic.
- Modal leader fallback and `Escape` close guard (`busy === "create-team"`) remain unchanged.

3. Reuse existing state setters.
- Hook accepts current page callbacks/setters instead of introducing new reducer actions.

## Validation

- `npm --prefix web run test -- src/pages/team/run_helpers.test.ts src/pages/team_panels.test.tsx`
- `npm --prefix web run lint`
- `npm --prefix web run build`

## Risks

1. Risk: More hook files can increase wiring surface.
- Mitigation: each hook has a strict lifecycle boundary (`run`, `mailbox`, `create-modal`) with explicit option typing.

2. Risk: Future dependency-array drift across hooks.
- Mitigation: keep each hook focused and colocated under `web/src/pages/team/` for easier targeted review.
