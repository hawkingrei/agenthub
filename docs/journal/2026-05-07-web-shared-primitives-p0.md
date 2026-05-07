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

## Team Panel Follow-Up

This follow-up continued the same P0 primitive rollout into the Team setup and
mailbox panels without changing Team behavior.

Additional scope:

- Move the setup checklist and copy-action button classes into
  `web/src/ui/tailwind_classes.ts`.
- Move mailbox meta, member-row, chat-header, message-body, composer, and
  advanced-control classes into shared constants.
- Add focused render assertions so the setup/mailbox panels keep using the
  shared constants instead of drifting back to inline Tailwind strings.
- Update the Tailwind audit and active TODO remaining estimate after the
  mailbox/setup slice.

Additional validation:

```bash
npm --prefix web run test -- src/pages/team_setup_panel.test.tsx src/pages/team_panels.test.tsx
```

Result:

- focused Vitest: `2` files, `98` tests passed

Chrome MCP/browser check:

- URL: `http://127.0.0.1:5173/workspace/teams`
- Chrome DevTools MCP was blocked because `list_pages` returned
  `selected page has been closed`.
- Browser automation fallback opened the local page, but the available session
  was unauthenticated and the app redirected to `/?next=/workspace/teams`.
- A temporary localStorage auth bypass reached the Teams route, but the Vite-only
  frontend returned the empty Teams shell plus an API JSON parse error because no
  backend API was attached to that browser check.

Remaining follow-ups:

- Continue Team shell/detail and Tier 2 panel migrations.
- Run a real authenticated Chrome MCP check against a backend-backed session for
  the Team setup/mailbox surfaces before closing the P0 browser-validation item.

## Team Tier 2 Panel Follow-Up

This follow-up moved the next Team Tier 2 panel class literals into the shared
Tailwind constants layer without changing rendering behavior.

Additional scope:

- Move Team run browser list, hint, subtitle, and footer metadata classes into
  `web/src/ui/tailwind_classes.ts`.
- Move Team steps panel layout, list, item, and notice classes into shared
  constants while preserving the existing semantic selectors used by tests.
- Move the member status row class into the shared constants layer.
- Add focused render assertions that lock these panels to the shared constants.

Additional validation:

```bash
npm --prefix web run test -- src/pages/team_panels.test.tsx src/pages/team_member_status_strip.test.tsx
cd web && npm exec tsc -- --noEmit
npm --prefix web run lint
git diff --check
```

Results:

- focused Vitest: `2` files, `103` tests passed
- TypeScript no-emit: pass
- lint: pass
- whitespace check: pass

Remaining follow-ups:

- Continue the remaining Tier 2 detail panels, especially active-run, member
  ACP, thread pane, sidebar, and management modal surfaces.
