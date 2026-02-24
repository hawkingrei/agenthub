# Team UI State Reducer Phase 1

## Summary

Introduce a first reducer-backed state slice in `TeamPage` for stable UI-only fields:
- `tab`
- `runLookupId`
- `eventsAutoRefresh`

## Background

`team_page.tsx` historically used many independent `useState` atoms. After panel extraction, migrating low-coupling UI fields into a reducer is a safe first step toward a broader reducer model.

## Scope

- `web/src/pages/team_page.tsx`
- `docs/todo.md`

## Key Decisions

1. Keep reducer scope deliberately narrow (UI-only, no async mutation state).
2. Keep existing setter ergonomics by exposing callback wrappers (`setTab`, `setRunLookupId`, `setEventsAutoRefresh`) over dispatch.
3. Avoid behavior changes while preparing for future reducer consolidation.

## Validation

Executed (2026-02-19):

```bash
npm --prefix web run lint
npm --prefix web run test -- src/pages/team_page.runs.test.ts
npm --prefix web run build
/bin/zsh -lc 'set -euo pipefail; npm --prefix web run dev -- --host 127.0.0.1 --port 5173 --strictPort >/tmp/agenthub-vite.log 2>&1 & VITE_PID=$!; trap "kill $VITE_PID 2>/dev/null || true" EXIT; for i in {1..30}; do curl -sf http://127.0.0.1:5173 >/dev/null && break; sleep 1; done; PLAYWRIGHT_NO_WEBSERVER=1 npm --prefix web run e2e -- tests/e2e/team_page.e2e.ts'
```
