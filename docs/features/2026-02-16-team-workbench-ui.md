# Team Workbench UI

## Background

Team backend APIs (`/api/teams`, run lifecycle, step lifecycle, actor mailbox) are already available,
but there is no first-class UI to operate these flows from the web app.

## Scope

- Add a dedicated Team Workbench page at `/teams`.
- Add Team API types and request wrappers in `web/src/api.ts`.
- Add Team entry in the main authenticated header.
- Implement interactive flows:
  - Team create and team list selection
  - Run create, run lookup by `run_id`, run status refresh, run cancel
  - Run event timeline refresh and older-page replay (`before_id` pagination)
  - Step submit + lifecycle transitions (`start`, `complete`, `fail`, `input_required`, `resume`)
  - Actor mailbox send/inbox/ack operations
  - Team run list loading through `GET /api/teams/:id/runs`

## Key Decisions

- Keep Team interactions in a standalone page (`web/src/pages/team_page.tsx`) to avoid coupling
  with the existing Agent workspace state machine.
- Use backend `list runs by team` API as the source of truth for run list rendering; `load by run_id`
  remains available for fast jump-to-run workflows.
- Keep JSON-first input UX for spec/input/payload/route to match backend contracts exactly
  and unblock debugging.

## Validation

Executed (2026-02-18):

```bash
npm --prefix web run test -- src/pages/team_page.runs.test.ts
/bin/zsh -lc 'set -euo pipefail; npm --prefix web run dev -- --host 127.0.0.1 --port 5173 --strictPort >/tmp/agenthub-vite.log 2>&1 & VITE_PID=$!; trap "kill $VITE_PID 2>/dev/null || true" EXIT; for i in {1..30}; do curl -sf http://127.0.0.1:5173 >/dev/null && break; sleep 1; done; PLAYWRIGHT_NO_WEBSERVER=1 npm --prefix web run e2e -- tests/e2e/team_page.e2e.ts'
npm --prefix web run build
```
