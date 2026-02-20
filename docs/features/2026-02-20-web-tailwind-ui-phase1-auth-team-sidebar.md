# Web Tailwind UI Phase-1: Auth + Team Sidebar

## Background

After introducing Tailwind baseline tooling, the next step is to migrate a small,
high-visibility UI slice to utility classes while keeping legacy styles and
component behavior intact.

This phase focuses on:

- auth/login card presentation;
- join-page form controls;
- Team sidebar `Team Forge` entry block (`Guided Wizard` / `Manual Spec`).

## Scope

- `web/src/app.tsx`
- `web/src/pages/auth_pages.tsx`
- `web/src/pages/join_page.tsx`
- `web/src/pages/team_sidebar.tsx`
- `docs/todo.md`

## Key Decisions

1. Keep existing semantic class names (for compatibility) and layer Tailwind
   utility classes on top.
2. Prefer utility classes for spacing, border, background, and focus states in
   migrated sections.
3. Limit migration to leaf UI shells in this phase; avoid changing reducer/data
   flow logic or interaction contracts.
4. Keep button semantics and callbacks unchanged (`Refresh`, `Guided Wizard`,
   `Manual Spec`, login/join actions).

## Validation Evidence (local)

- Build:
  - `npm --prefix web run build`

## Follow-up Validation

- Manual visual regression checks on desktop/mobile:
  - `/` login card, inputs, and bootstrap/login actions;
  - `/join` join form card and inputs;
  - `/teams` sidebar card, Team Forge entry actions, and team list interactions.
- Run focused component tests when touching panel layout again:
  - `npm --prefix web run test -- src/pages/team_panels.test.tsx`

## Notes

- This phase intentionally does not remove legacy CSS blocks yet.
- Subsequent phases can migrate `TeamRunPanel` and mailbox/event panels with the
  same compatibility-first approach.
