# Web Tailwind UI Phase-2: TeamRunPanel

## Background

After phase-1 migration of auth/join and Team sidebar forge entry, the next
high-traffic area is `TeamRunPanel`, which is the primary run control surface in
`/teams`.

## Scope

- `web/src/pages/team_run_panel.tsx`
- `docs/todo.md`

## Key Decisions

1. Keep existing interaction contracts unchanged:
   - `Create Run`, `Load Run`, `Refresh Runs`, `Load More`, and run-item selection
     callbacks are unchanged.
2. Keep semantic legacy class names where they already exist and layer Tailwind
   utility classes for:
   - card/panel shell;
   - toolbar/action buttons;
   - text inputs/textarea/select focus states;
   - run-list container and head actions.
3. Keep migration strictly visual:
   - no reducer/api/state shape changes;
   - no behavioral changes to run filter and pagination semantics.

## Validation Evidence (local)

- Focused panel tests:
  - `npm --prefix web run test -- src/pages/team_panels.test.tsx`
- Lint:
  - `npm --prefix web run lint`
- Build:
  - `npm --prefix web run build`

## Follow-up Validation

- Manual desktop/mobile checks in `/teams`:
  - team header + delete action visual states;
  - create/load form control spacing and focus ring behavior;
  - run filter select and refresh/load-more disabled states;
  - active run item emphasis remains clear.

## Notes

- This phase keeps legacy CSS compatibility and does not remove existing
  `styles.css` blocks yet.
