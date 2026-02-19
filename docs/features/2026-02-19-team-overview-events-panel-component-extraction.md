# Team Overview And Events Panel Component Extraction

## Summary

Extract Team `overview` and `events` tab rendering from `web/src/pages/team_page.tsx` into dedicated components:
- `TeamOverviewPanel`
- `TeamEventsPanel`

## Background

After extracting sidebar/run/mailbox/member-console panels, `team_page.tsx` still inlined `overview` and `events` tab trees. These tabs are relatively self-contained and can be moved without changing data flow ownership.

## Scope

- `web/src/pages/team_page.tsx`
- `web/src/pages/team_overview_panel.tsx`
- `web/src/pages/team_events_panel.tsx`
- `docs/todo.md`

## Key Decisions

1. Keep parent-owned state and async operations in `TeamPage`:
   - snapshot refresh,
   - events refresh/load-older,
   - auto-refresh toggle source of truth.
2. Add explicit parent callbacks to replace inline handlers:
   - `onRefreshOverviewSnapshot`
   - `onOpenMailboxForMember`
   - `onRefreshEventsPanel`
   - `onLoadOlderEventsPanel`
3. Preserve behavior exactly:
   - clicking member in overview still jumps to mailbox tab,
   - events preview text and load-older gating unchanged.

## Validation

Executed (2026-02-19):

```bash
npm --prefix web run lint
npm --prefix web run test -- src/pages/team_page.runs.test.ts
npm --prefix web run build
/bin/zsh -lc 'set -euo pipefail; npm --prefix web run dev -- --host 127.0.0.1 --port 5173 --strictPort >/tmp/agenthub-vite.log 2>&1 & VITE_PID=$!; trap "kill $VITE_PID 2>/dev/null || true" EXIT; for i in {1..30}; do curl -sf http://127.0.0.1:5173 >/dev/null && break; sleep 1; done; PLAYWRIGHT_NO_WEBSERVER=1 npm --prefix web run e2e -- tests/e2e/team_page.e2e.ts'
```
