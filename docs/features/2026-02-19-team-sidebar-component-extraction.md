# Team Sidebar Component Extraction

## Summary

Extract the Team list/entry sidebar from `web/src/pages/team_page.tsx` into a dedicated `TeamSidebar` component to reduce render complexity while preserving all existing behavior.

## Background

`team_page.tsx` previously inlined sidebar and main workbench rendering in one large component. After extracting mailbox/member-console tabs, sidebar remained another isolated UI slice with clear boundaries (team list, refresh, Team Forge entry). Extracting it is the next low-risk maintainability step.

## Scope

- `web/src/pages/team_page.tsx`
- `web/src/pages/team_sidebar.tsx`
- `docs/todo.md`

## Key Decisions

1. Keep state ownership in `TeamPage`:
   - selected team state,
   - team summary map,
   - draft metadata used in Team Forge launch card.
2. Keep sidebar component callback-driven:
   - `onRefreshTeams`,
   - `onOpenCreateTeamModal`,
   - `onSelectTeam`.
3. Preserve exact UX semantics:
   - selecting a team still clears `runLookupId`,
   - create-team launch card still shows draft team/leader/worker metadata.

## Validation

Executed (2026-02-19):

```bash
npm --prefix web run lint
npm --prefix web run test -- src/pages/team_page.runs.test.ts
npm --prefix web run build
/bin/zsh -lc 'set -euo pipefail; npm --prefix web run dev -- --host 127.0.0.1 --port 5173 --strictPort >/tmp/agenthub-vite.log 2>&1 & VITE_PID=$!; trap "kill $VITE_PID 2>/dev/null || true" EXIT; for i in {1..30}; do curl -sf http://127.0.0.1:5173 >/dev/null && break; sleep 1; done; PLAYWRIGHT_NO_WEBSERVER=1 npm --prefix web run e2e -- tests/e2e/team_page.e2e.ts'
```
