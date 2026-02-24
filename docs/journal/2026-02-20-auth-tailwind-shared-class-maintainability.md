# Auth Tailwind Shared Class Maintainability Follow-up

## Background

Review feedback flagged duplicated Tailwind class strings across login/join/auth
shell pages (`App`, `JoinPage`, `AuthRequired`/`ForbiddenPage`). The duplication
increased drift risk for future style updates.

## Scope

- `web/src/ui/tailwind_classes.ts`
- `web/src/app.tsx`
- `web/src/pages/join_page.tsx`
- `web/src/pages/auth_pages.tsx`
- `AGENTS.md`

## Key Decisions

1. Add a shared Tailwind class constants module for auth-related shell styles:
   - page shell;
   - auth card base and form variant;
   - input/button/action-row classes.
2. Replace duplicated local constants in login/join/auth pages with imports from
   the shared module.
3. Keep Join page's top margin behavior by composing
   `JOIN_PRIMARY_BUTTON_CLASS = `mt-1 ${AUTH_PRIMARY_BUTTON_CLASS}``.
4. Add a project policy line in `AGENTS.md`:
   - low-risk maintainability review suggestions should be applied directly in
     the active change by default.

## Validation

- Frontend build:
  - `npm --prefix web run build`
- Frontend lint:
  - `npm --prefix web run lint`

## Notes

- This change is refactor/policy focused and does not alter auth flow logic or
  backend API behavior.
