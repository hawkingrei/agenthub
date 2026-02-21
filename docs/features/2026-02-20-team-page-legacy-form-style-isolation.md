# Team Page Legacy Form Style Isolation

## Background

After Tailwind v4 migration, the Teams workbench still inherited legacy global form-control selectors from `web/src/styles.css`:

- `button:not([class*="mantine-"])`
- `input:not([class*="mantine-"])`
- `textarea:not([class*="mantine-"])`
- `select:not([class*="mantine-"])`

These selectors changed button/input appearance and spacing on `/teams`, which caused layout drift compared with the intended Tailwind utility styling.

## Scope

- `web/src/pages/team_page.tsx`
- `web/src/styles.css`
- `docs/todo.md`

## Key Decisions

1. Isolate Teams page from legacy global form-control selectors:
   - add runtime body class `teams-page` while `TeamPage` is mounted.
   - scope legacy selectors with `body:not(.teams-page)` so they continue to work for legacy pages but stop overriding Teams Tailwind controls.
2. Reduce legacy tag-level layout side effects on Teams shell:
   - remove legacy `app` class from Teams root container.
   - replace Teams layout wrapper from `<section>` to `<div>`.
   - normalize header/title behavior with explicit utility classes (`mb-0`, `whitespace-normal`).
3. Keep change set minimal and reversible:
   - no API/data behavior changes.
   - no large CSS block additions.
## Validation

- `npm --prefix web run test -- src/pages/team_page.runs.test.ts src/pages/team_panels.test.tsx`
- `npm --prefix web run build`

All checks passed locally.
