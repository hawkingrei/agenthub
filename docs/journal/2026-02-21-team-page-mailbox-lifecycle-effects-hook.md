# Team Page Mailbox Lifecycle Effects Hook

## Summary

Improve `TeamPage` maintainability by extracting mailbox/member lifecycle side effects into a dedicated hook while keeping behavior unchanged.

## Background

After run-lifecycle extraction in PR1, `web/src/pages/team_page.tsx` still contained several mailbox/member-focused `useEffect` blocks:

- snapshot-driven selected-member fallback,
- inbox auto-load on active run + conversation actor,
- mailbox auto-scroll / seen-marker sync,
- member-events refresh trigger.

Those effects were mixed with render and action handlers, which increased file density and made future edits harder to review.

## Scope

- `web/src/pages/team/use_team_mailbox_lifecycle_effects.ts` (new)
- `web/src/pages/team_page.tsx`

## Key Decisions

1. Extract only mailbox/member lifecycle effects in this PR.
- Keep create-modal and runtime-default effects in `TeamPage` for a later incremental slice.

2. Preserve existing callback contracts.
- Hook inputs reuse existing callbacks (`loadInbox`, `loadMemberEvents`, `markConversationSeen`, scroll handler) to avoid behavior drift.

3. Keep error semantics unchanged.
- Hook still maps async failures through the same `parseErrorMessage` + `setError` path used before extraction.

## Validation

- `npm --prefix web run test -- src/pages/team/run_helpers.test.ts src/pages/team_panels.test.tsx`
- `npm --prefix web run lint`
- `npm --prefix web run build`

## Risks

1. Risk: Hook parameter surface can grow.
- Mitigation: extraction boundary is explicitly limited to mailbox/member effects and avoids unrelated state/control logic.

2. Risk: Dependency array drift in follow-up edits.
- Mitigation: centralize these effects in one hook file and keep call-site wiring explicit in `TeamPage`.
