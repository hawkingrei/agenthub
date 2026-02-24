# Web Tailwind v4 Border Color Compatibility Guard

## Background

After upgrading from Tailwind v3 to v4, several UI surfaces started showing unexpected dark border frames.

Root cause: Tailwind v4 preflight changed the default border color fallback to `currentColor`.
During migration, elements using generic `border` (or inheriting border behavior) without an explicit `border-*` color can therefore render dark borders.

## Scope

- `web/src/tailwind.css`
- `docs/todo.md`

## Key Decisions

1. Add a base-layer compatibility rule in Tailwind entry CSS:
   - set default border color to neutral slate (`--color-slate-200` fallback `#e2e8f0`) for
     `*`, pseudo-elements, `::backdrop`, and `::file-selector-button`.
2. Keep explicit `border-*` color utilities untouched:
   - component-level border color utilities still override this base fallback.
3. Keep migration low-risk:
   - avoid large component-by-component border rewrites in this step.

## Validation

- `npm --prefix web run build`
- `npm --prefix web run lint`
- `npm --prefix web run test -- src/pages/team_panels.test.tsx src/pages/team_page.runs.test.ts`

All checks passed locally.
