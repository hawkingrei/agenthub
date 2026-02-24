# Web Tailwind UI Phase-3: Team Overview/Events/Steps Panels

## Background

After phase-2 (`TeamRunPanel`) migration, the next UI-heavy surfaces in Team
workbench are `TeamOverviewPanel`, `TeamEventsPanel`, and `TeamStepsPanel`.

These panels are rich in controls and list rendering, but their behavior is
already validated by existing panel tests, so this phase focuses on visual-layer
migration only.

## Scope

- `web/src/pages/team_overview_panel.tsx`
- `web/src/pages/team_events_panel.tsx`
- `web/src/pages/team_steps_panel.tsx`
- `docs/todo.md`

## Key Decisions

1. Keep all callbacks and data flow untouched:
   - snapshot refresh
   - event refresh/load-older/auto-refresh toggle
   - step submit/action refresh and per-action payload inputs
2. Layer Tailwind utility classes on top of existing semantic classes for:
   - panel shell and toolbar alignment
   - button hierarchy (primary/secondary)
   - input/select/textarea focus states
   - list containers and surface grouping
3. Preserve legacy class names for compatibility with existing CSS and tests.

## Validation Evidence (local)

- Focused panel tests:
  - `npm --prefix web run test -- src/pages/team_panels.test.tsx`
- Lint:
  - `npm --prefix web run lint`
- Build:
  - `npm --prefix web run build`

## Follow-up Validation

- Manual desktop/mobile checks in `/teams`:
  - Overview member list active-row visibility and snapshot meta readability
  - Events auto-refresh toggle + load-older disabled states
  - Steps submit/action form usability and list readability under long content

## Notes

- This phase intentionally avoids reducer/state changes and keeps migration
  incremental.
