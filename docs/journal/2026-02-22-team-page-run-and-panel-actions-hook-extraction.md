# Team Page Run/Panel Actions Hook Extraction

## Summary

Refactor `TeamPage` callback-heavy sections into dedicated hooks to reduce file size and keep action responsibilities localized:
- `use_team_run_list_actions.ts`
- `use_team_run_lifecycle_actions.ts`
- `use_team_step_actions.ts`
- `use_team_mailbox_actions.ts`
- `use_team_panel_actions.ts`
- `use_team_run_effects.ts` (run bootstrap + auto-refresh effects)
- `use_team_mailbox_effects.ts` (mailbox/member selection + conversation follow effects)
- `use_team_member_backfill_effect.ts` (team member agent backfill lookup effect)
- `use_team_create_modal_effects.ts` (runtime defaults + create-modal keyboard/leader fallback effects)
- `use_team_refs_effects.ts` (event/run/member ref synchronization effects)

## Background

`web/src/pages/team_page.tsx` remained large and still carried many inline callbacks for run list controls, run lifecycle operations, step actions, mailbox actions, panel refresh actions, and run bootstrap effects. This increased review cost and made dependency arrays harder to reason about during follow-up refactors.

## Scope

- `web/src/pages/team_page.tsx`
- `web/src/pages/team/use_team_run_list_actions.ts`
- `web/src/pages/team/use_team_run_lifecycle_actions.ts`
- `web/src/pages/team/use_team_step_actions.ts`
- `web/src/pages/team/use_team_mailbox_actions.ts`
- `web/src/pages/team/use_team_panel_actions.ts`
- `web/src/pages/team/use_team_run_effects.ts`
- `web/src/pages/team/use_team_mailbox_effects.ts`
- `web/src/pages/team/use_team_member_backfill_effect.ts`
- `web/src/pages/team/use_team_create_modal_effects.ts`
- `web/src/pages/team/use_team_refs_effects.ts`
- `web/src/pages/team/use_team_run_list_actions.test.tsx`
- `web/src/pages/team/use_team_run_lifecycle_actions.test.tsx`
- `web/src/pages/team/use_team_step_actions.test.tsx`
- `web/src/pages/team/use_team_run_effects.test.tsx`
- `web/src/pages/team/use_team_mailbox_effects.test.tsx`
- `docs/todo.md`

## Key Decisions

1. Keep panel component callback signatures unchanged, so extraction is implementation-only and does not require panel API changes.
2. Group callbacks by ownership boundary:
   - run list filter/refresh/load-more
   - run lifecycle (load/cancel/resume/restart)
   - step submit/action transitions
   - mailbox chat/raw message/inbox ack flow
   - overview/events/member-console refresh/jump actions
3. Extract run bootstrap effects (`initial refresh`, `team switch`, `active run bootstrap`, `auto refresh`) into a dedicated hook to keep page-level effect arrays short and easier to audit.
4. Extract mailbox/member effects (`snapshot member default`, `inbox actor sync`, `mailbox scroll follow`, `member event initial load`) into a dedicated hook to isolate conversation behavior from page shell rendering.
5. Extract residual page effects (member backfill lookup, runtime defaults + modal keydown, and ref synchronization) so `TeamPage` becomes orchestration-only and effect dependencies are isolated by concern.
6. Preserve existing error/busy semantics (`setError`, `setBusy`) and run-scoped refresh behavior to avoid UI behavior drift.

## Validation

Executed (2026-02-22):

```bash
pnpm -C web exec eslint src/pages/team_page.tsx src/pages/team/use_team_run_list_actions.ts src/pages/team/use_team_run_lifecycle_actions.ts src/pages/team/use_team_step_actions.ts src/pages/team/use_team_mailbox_actions.ts src/pages/team/use_team_panel_actions.ts src/pages/team/use_team_run_effects.ts src/pages/team/use_team_mailbox_effects.ts src/pages/team/use_team_member_backfill_effect.ts src/pages/team/use_team_create_modal_effects.ts src/pages/team/use_team_refs_effects.ts src/pages/team/use_team_run_list_actions.test.tsx src/pages/team/use_team_run_lifecycle_actions.test.tsx src/pages/team/use_team_step_actions.test.tsx src/pages/team/use_team_run_effects.test.tsx src/pages/team/use_team_mailbox_effects.test.tsx
pnpm -C web test -- src/pages/team/use_team_run_list_actions.test.tsx
pnpm -C web test -- src/pages/team/use_team_run_lifecycle_actions.test.tsx
pnpm -C web test -- src/pages/team/use_team_step_actions.test.tsx
pnpm -C web test -- src/pages/team/use_team_mailbox_actions.test.tsx
pnpm -C web test -- src/pages/team/use_team_create_modal_effects.test.tsx src/pages/team/use_team_member_backfill_effect.test.tsx
pnpm -C web test -- src/pages/team/use_team_run_effects.test.tsx src/pages/team/use_team_mailbox_effects.test.tsx
pnpm -C web test -- src/pages/team
pnpm -C web build
```
