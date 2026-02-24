# Team Workbench Post-Load Layout Collapse Guard

## Background

In Team workbench, first paint looked clean, but after follow-up async loading/refresh cycles some panels could appear visually collapsed or stacked.
This was caused by legacy global layout styles (`toolbar/actions/tab-bar` and generic `.card ul` limits) leaking into the Team page shell.

## Scope

- `web/src/pages/team_page.tsx`
- `web/src/ui/tailwind_classes.ts`
- `web/src/styles.css`
- `docs/todo.md`

## Key Decisions

1. Remove Team Active Run shell reliance on legacy global classes:
   - Stop using `toolbar/actions/tab-bar` class hooks in `TeamPage` active-run header/tabs.
   - Keep equivalent spacing/flow directly in Tailwind utility classes.
2. Flatten Team Debug strip layering:
   - Remove unnecessary `z-index` wrappers in Debug tools head.
   - Keep normal document flow so debug tags do not visually overlap adjacent content after refresh.
3. Scope generic list height constraints away from Team cards:
   - Change global `.card ul` max-height rule to `.admin .card ul` only.
   - Prevent Team run/event/message lists from being unexpectedly constrained/collapsed after content loads.
4. Align shared Team panel constants toward Tailwind-only layout:
   - `TEAM_PANEL_TOOLBAR_CLASS` and `TEAM_PANEL_TOOLBAR_ACTIONS_CLASS` no longer include legacy global class names.

## Validation Evidence (2026-02-20)

- `npm --prefix web run lint`
- `npm --prefix web run test -- src/pages/team_panels.test.tsx src/pages/team_page.runs.test.ts`

## Notes

- This is a layout-stability hardening change.
- No backend behavior or Team run lifecycle semantics were changed.
