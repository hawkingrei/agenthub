# Team Workbench Selected-Team Run Scope

## Background

Team operators can switch between many teams from the left sidebar.
Previously, run-level actions could still follow a globally selected run and were not fully constrained by the currently selected team, which caused accidental cross-team context confusion.

## Scope

- `web/src/pages/team_page.tsx`
- `web/src/pages/team_run_panel.tsx`
- `docs/todo.md`

## Key Decisions

1. Bind the active run context to the selected team:
   - Introduce team-scoped active run derivation (`activeRunForSelectedTeam` / `activeRunIdForSelectedTeam`).
   - All major workbench output rendering now depends on the selected team's active run only.
2. Enforce team-bound run actions:
   - `Load Run` rejects run IDs that belong to another team.
   - `Resume Run` / `Restart Run` / `Cancel Run` require a run in the current team.
   - Remove implicit team-switch behavior during run lookup/load.
3. Enforce team-bound step/mailbox actions:
   - Step submit/action, mailbox send/chat/ack/inbox refresh all execute against the selected team's active run ID.
   - Error copy now explicitly asks for selecting a run in the current team.
4. Align run panel copy with behavior:
   - `Load Run` guidance text now states it works within the currently selected team.

## Validation Evidence (2026-02-20)

- `npm --prefix web run test -- src/pages/team_panels.test.tsx src/pages/team_page.runs.test.ts`
- `npm --prefix web run lint`

## Notes

- This change focuses on interaction scoping and safety.
- It does not alter backend run state model or run status transitions.
