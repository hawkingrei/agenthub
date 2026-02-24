# Team Run Panel Component Extraction

## Summary

Extract Team run control/list UI from `web/src/pages/team_page.tsx` into `TeamRunPanel` to further reduce page complexity while keeping the same run-management behavior.

## Background

After extracting sidebar and mailbox/member-console panels, `team_page.tsx` still contained a large mixed block for:
- team member live status strip,
- create/load run controls,
- run status filter + paging list.

This block has clear boundaries and callback contracts, so it is a suitable next extraction step.

## Scope

- `web/src/pages/team_page.tsx`
- `web/src/pages/team_run_panel.tsx`
- `docs/todo.md`

## Key Decisions

1. Keep state and async orchestration in `TeamPage`:
   - run browser cursor/filter state,
   - create/load run actions,
   - refresh/load-more flows.
2. `TeamRunPanel` remains presentational + callback-driven.
3. Convert inlined run filter/refresh handlers into explicit callbacks in `TeamPage`:
   - `onRunStatusFilterChange`,
   - `onRefreshRuns`.

## Validation

Executed (2026-02-19):

```bash
npm --prefix web run lint
npm --prefix web run test -- src/pages/team_page.runs.test.ts
npm --prefix web run build
/bin/zsh -lc 'set -euo pipefail; npm --prefix web run dev -- --host 127.0.0.1 --port 5173 --strictPort >/tmp/agenthub-vite.log 2>&1 & VITE_PID=$!; trap "kill $VITE_PID 2>/dev/null || true" EXIT; for i in {1..30}; do curl -sf http://127.0.0.1:5173 >/dev/null && break; sleep 1; done; PLAYWRIGHT_NO_WEBSERVER=1 npm --prefix web run e2e -- tests/e2e/team_page.e2e.ts'
```
