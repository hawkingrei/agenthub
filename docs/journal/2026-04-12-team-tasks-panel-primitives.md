# Team Tasks Panel Primitive Follow-up

## Summary

Continued the shared Team UI primitive rollout on `web/src/pages/team_tasks_panel.tsx`
after the `TeamTaskPanel` cleanup, focusing on repeated lane-count and empty-state
shells instead of changing task lifecycle behavior.

## Changed

- `web/src/pages/team_tasks_panel.tsx`
  - switched lane count badges to the shared `Badge` primitive
  - switched previous-run count badges to the shared `Badge` primitive
  - switched board loading / empty / no-result affordances to the shared `EmptyState` primitive
- `web/src/pages/team_panels.test.tsx`
  - added regression coverage for the empty task-board state

## Validation

- `cd web && npm run test -- src/ui/primitives.test.tsx src/pages/team_panels.test.tsx`
- `cd web && npm run lint -- src/ui/primitives.tsx src/ui/primitives.test.tsx src/pages/team_task_panel.tsx src/pages/team_tasks_panel.tsx src/pages/team_panels.test.tsx`
- `cd web && npm run build`
