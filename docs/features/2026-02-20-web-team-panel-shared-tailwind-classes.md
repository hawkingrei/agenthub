# Web Team Panel Shared Tailwind Classes

## Background

Team workbench panels (`TeamRunPanel`, `TeamOverviewPanel`, `TeamEventsPanel`,
`TeamStepsPanel`) kept repeating the same Tailwind class strings for shell,
buttons, inputs, and toolbar layout. This created drift risk during the ongoing
UI migration.

## Scope

- `web/src/ui/tailwind_classes.ts`
- `web/src/pages/team_run_panel.tsx`
- `web/src/pages/team_overview_panel.tsx`
- `web/src/pages/team_events_panel.tsx`
- `web/src/pages/team_steps_panel.tsx`
- `docs/todo.md`

## Key Decisions

1. Extract Team panel shared style constants into
   `web/src/ui/tailwind_classes.ts`:
   - card shell;
   - toolbar and toolbar actions;
   - primary/secondary buttons;
   - input/textarea controls;
   - panel title typography.
2. Rewire four Team panel components to consume the shared constants without
   changing existing callbacks, API contracts, or reducer state flow.
3. Keep existing semantic class names (`teams-*`, `team-item`, etc.) so legacy
   compatibility CSS and current tests remain stable during incremental migration.
4. Add extra utility layering for phase-3 panels to improve readability in list
   surfaces (event rows, step rows, overview meta blocks) without changing
   behavior.

## Validation Evidence (local)

- `npm --prefix web run test -- src/pages/team_panels.test.tsx`
- `npm --prefix web run lint`
- `npm --prefix web run build`

## Notes

- This change is maintainability + visual-layer migration only.
- No Team run/step/event/mailbox business logic changed.
