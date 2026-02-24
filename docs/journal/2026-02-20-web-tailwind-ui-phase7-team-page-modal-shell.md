# Web Tailwind UI Phase-7: TeamPage Shell + Team Forge Modal

## Background

After phases 1-6, the largest remaining legacy-styled surface in Team Workbench
was the `TeamPage` shell and the in-page `Team Forge` modal.

This phase layers Tailwind utility classes over those containers so the Team
entry flow matches the ongoing UI migration direction while keeping existing
wizard logic unchanged.

## Scope

- `web/src/pages/team_page.tsx`
- `docs/todo.md`

## Key Decisions

1. Keep all Team create/run behavior unchanged:
   - create stage progression and lock rules
   - manual spec jump behavior
   - worker duplicate handling and resolve flow
2. Preserve existing semantic class names (for backward compatibility with
   existing CSS/tests), and add Tailwind utilities for layout/spacing/visual
   states.
3. Standardize modal controls with shared utility class constants inside
   `TeamPage`:
   - primary/secondary/ghost buttons
   - shared field styles for `input`/`select`/`textarea`

## Validation Evidence (local)

- Focused tests:
  - `npm --prefix web run test -- src/pages/team_page.runs.test.ts src/pages/team_panels.test.tsx`
- Lint:
  - `npm --prefix web run lint`
- Build:
  - `npm --prefix web run build`

## Follow-up Validation

- Manual desktop/mobile checks in `/teams`:
  - team header/session action readability
  - Team Forge stage button states (`active`/`completed`/`locked`)
  - modal form control spacing and disabled states
  - worker card actions and duplicate-warning visibility

## Notes

- This phase intentionally avoids changing reducer/state transitions, API
  payload generation, and Team tab business logic.
