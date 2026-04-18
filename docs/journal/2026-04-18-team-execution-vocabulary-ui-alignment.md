# Team Execution Vocabulary UI Alignment

## Summary

- tightened Team UI wording so `run` stays framed as a concrete execution partition instead of a
  generic work-progress synonym;
- kept the current API/runtime schema unchanged because `TeamRunRecord` still does not expose an
  additive `attempt_number` projection;
- limited this pass to user-facing Team run/task surfaces and regression coverage.

## Implementation Notes

- `web/src/pages/team_run_panel.tsx`
  - changed the run browser helper copy to describe runs as concrete execution history/replay
    partitions;
  - clarified the default CTA hint from "Start a new run" to "Start a new execution run";
  - clarified empty states to say "No execution runs loaded yet".
- `web/src/pages/team_active_run_panel.tsx`
  - renamed the card heading to `Active Execution Run`;
  - renamed the status metadata label to `Execution` to avoid treating the whole panel as a generic
    task-progress surface.
- `web/src/pages/team_tasks_panel.tsx`
  - clarified task detail copy to say `No execution run recorded yet`, `Latest execution run is
    still in progress`, and `Previous execution runs`.

## Validation

- `cd web && npm run test -- vite.config.test.ts src/pages/team_panels.test.tsx`

## Notes

- Chrome DevTools MCP remained unavailable in this session (`chrome-devtools/list_pages` returned
  `Transport closed`), so this change only has code-level regression evidence.
- The broader vocabulary follow-up in `docs/todo.md` remains open. A future additive
  `attempt_number` projection should only land once runtime semantics are explicit enough to avoid
  conflating "new run" with "new attempt".
