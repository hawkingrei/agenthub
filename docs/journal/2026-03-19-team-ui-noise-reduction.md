# Team UI Noise Reduction

## Summary

- reduced transport-heavy noise in `Teams -> all`
- moved Kanban debug affordances behind a collapsed developer-only disclosure
- continued the Team workspace visual direction toward a lighter review-first surface

## Channel Surface

`Teams -> all` should read as a communication/review lane instead of a delivery console.

Changes:

- replaced visible `Seen by 0 agents` copy with a lighter `Delivery pending` hint
- changed the empty copy from `No thread messages yet.` to `No channel messages yet.`

This keeps delivery visibility available without making every human message look like transport status.

## Kanban Detail

Task detail should stay task-first even in developer mode.

Changes:

- wrapped manual compile-preview controls inside a collapsed `Developer tools` disclosure
- kept compile preview, payload reuse, preview-based run creation, and raw task context available
- removed their default visual weight from the main task detail body

This keeps normal planning/execution reading focused on:

- task status
- latest run summary
- previous runs

## Validation

- `cd web && npx vitest run src/pages/team_panels.test.tsx`
- `cd web && npm run lint -- src/pages/team_task_panel.tsx src/pages/team_tasks_panel.tsx src/pages/team_panels.test.tsx`
- `cd web && npm run build`
