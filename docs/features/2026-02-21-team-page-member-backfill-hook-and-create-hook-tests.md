# Team Page Member Backfill Hook And Create-Modal Hook Tests

## Summary

Complete the next maintainability slice for `TeamPage` by extracting Team member-agent backfill lifecycle effect into a dedicated hook and adding focused tests for `useTeamCreateModalLifecycleEffects`.

## Background

After run/mailbox/create-modal lifecycle extraction, `TeamPage` still kept one non-trivial side effect inline:

- Team spec member-agent backfill via `api.getAgent` for members hidden from `/api/agents`.

In parallel, create-modal hook extraction introduced a new hook file without direct focused tests, which kept patch coverage noisy in review.

## Scope

- `web/src/pages/team/use_team_member_agent_backfill_effect.ts` (new)
- `web/src/pages/team/use_team_create_modal_lifecycle_effects.test.tsx` (new)
- `web/src/pages/team_page.tsx`
- `docs/todo.md`

## Key Decisions

1. Keep extraction boundary narrow.
- Move only member-agent backfill effect logic to a dedicated hook.
- Do not change run/mailbox/create flow behavior.

2. Add focused hook tests instead of broad page-level tests.
- Cover create-modal hook branches:
  - empty token fallback root,
  - runtime default root fetch + normalization,
  - leader fallback on modal open,
  - Escape close behavior with busy guard.

3. Preserve existing contracts.
- Reuse existing page state setters and API client methods.

## Validation

- `npm --prefix web run test -- src/pages/team/run_helpers.test.ts src/pages/team_panels.test.tsx src/pages/team/use_team_create_modal_lifecycle_effects.test.tsx`
- `npm --prefix web run lint`
- `npm --prefix web run build`

## Risks

1. Risk: additional hook wiring surface.
- Mitigation: keep hook options typed and scoped to one lifecycle concern.

2. Risk: async effect test fragility.
- Mitigation: keep tests branch-focused with explicit microtask flushes and avoid timing assumptions.
