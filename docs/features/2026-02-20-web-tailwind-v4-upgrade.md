# Web Tailwind CSS v4 Upgrade

## Background

The web frontend used Tailwind CSS v3 (`tailwindcss@^3.4.17`) with legacy PostCSS plugin wiring (`tailwindcss` key in `postcss.config.cjs`).
To align with the current Tailwind release line and reduce future migration overhead, we upgraded to Tailwind v4.

## Scope

- `web/package.json`
- `web/package-lock.json`
- `web/postcss.config.cjs`
- `docs/todo.md`

## Key Decisions

1. Upgrade Tailwind dependency to v4 latest available in npm at upgrade time:
   - `tailwindcss@^4.2.0`
2. Add Tailwind v4 PostCSS plugin package:
   - `@tailwindcss/postcss@^4.2.0`
3. Migrate PostCSS plugin wiring from:
   - `tailwindcss: {}`
   to:
   - `"@tailwindcss/postcss": {}`
4. Keep existing Tailwind entry/config shape unchanged for now (`src/tailwind.css`, `tailwind.config.cjs`) to minimize migration blast radius.

## Validation

- `npm --prefix web run build`
- `npm --prefix web run lint`
- `npm --prefix web run test -- src/pages/team_panels.test.tsx src/pages/team_page.runs.test.ts`

All checks passed locally after dependency and PostCSS plugin migration.
