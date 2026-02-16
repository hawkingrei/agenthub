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

## Key Decisions

- Keep Team interactions in a standalone page (`web/src/pages/team_page.tsx`) to avoid coupling
  with the existing Agent workspace state machine.
- Because there is no `list runs by team` HTTP API yet, the UI supports:
  - creating runs,
  - loading existing runs by `run_id`,
  - keeping a browser-session run list for quick switching.
- Keep JSON-first input UX for spec/input/payload/route to match backend contracts exactly
  and unblock debugging.

## Validation

Suggested checks:

```bash
npm --prefix web run lint
npm --prefix web run test
```

Manual checks:

1. Create a team with valid JSON spec.
2. Create a run and verify status/event updates.
3. Submit a step and drive it through lifecycle transitions.
4. Send actor message, fetch inbox, and ack it.
5. Verify `/teams` can load an existing run by `run_id` after page refresh.
