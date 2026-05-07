# Web Shared Primitives P0 Rollout

## Summary

Continued the P0 frontend shared-primitives cleanup by moving repeated workspace
and Team shell Tailwind literals into `web/src/ui/tailwind_classes.ts`.

## Background

`docs/journal/2026-05-02-web-tailwind-audit.md` still tracked several repeated
route-shell and Team setup patterns after earlier primitive passes. This slice
keeps the migration focused on shared layout constants instead of changing Team
state, API behavior, or workspace information architecture.

## Scope

- Extract shared workspace/root/loading layout constants.
- Extract Team setup info-strip and action-grid constants.
- Migrate the current callers that used the repeated literals directly.
- Update the Tailwind audit to reflect the new post-change state.

## Key Decisions

- Keep these as Tailwind class constants rather than React components because
  the migrated call sites still own distinct content structure.
- Preserve existing Team task-first and workspace-shell behavior; this is a
  visual maintainability pass only.
- Leave larger mailbox and remaining Team panel migrations as follow-up work so
  this PR stays reviewable.

## Validation

Commands run:

```bash
cd web && npm run test -- src/ui/primitives.test.tsx src/pages/team_panels.test.tsx src/agents_root_page.test.tsx
cd web && npm run lint
cd web && npm exec tsc -- --noEmit
cd web && npm run build
```

Results:

- focused Vitest: `3` files, `112` tests passed
- lint: pass
- TypeScript no-emit: pass
- production build: pass

Chrome DevTools MCP:

- URL: `http://127.0.0.1:5173/workspace/nodes`
- Snapshot: unauthenticated route renders the Login shell instead of a blank page
- Console: Vite/React development messages plus one `404` resource request; no
  React render error observed

## Follow-Ups

- Continue migrating `team_setup_panel.tsx`, `team_mailbox_panel.tsx`, and the
  remaining Team detail surfaces onto shared primitives/constants.
- Keep the active TODO estimate aligned with the audit after each frontend
  cleanup PR.
