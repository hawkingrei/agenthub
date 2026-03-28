# Summary

Moved Team shared-thread discovery from a paginated task-list heuristic to a dedicated canonical
backend target.

This supersedes the earlier listing-only mitigation from
`2026-03-28-team-shared-thread-listing.md`.

## Why

The previous fix kept `bootstrap_kind=shared_thread` visible by adding an
`include_shared_thread` task-list query flag, but the Team page still inferred `# all` from a
`list_tasks(limit=100)` result. That remained fragile:

- a busy Kanban board could still push `# all` out of the top 100 tasks because shared-thread
  message writes do not advance the task list ordering signal;
- legacy data could already contain duplicate `all` / `shared_thread` tasks, and the client had no
  stable way to decide which one was canonical.

## Changes

- Added dedicated public Team shared-thread endpoints:
  - `GET /api/teams/:id/shared_thread`
  - `POST /api/teams/:id/shared_thread`
- Centralized shared-thread canonicalization in backend selection logic so all call sites reuse the
  same rule:
  - prefer the shared thread with the newest persisted conversation message;
  - if no shared-thread messages exist yet, fall back to the oldest created shared-thread record.
- Updated mailbox and run-side shared-thread resolution to reuse the same canonical selector.
- Updated the Team page so `Conversation` no longer derives `# all` from the Kanban task list.
- Kept workspace task refresh independent from shared-thread refresh.
- Removed the now-unused frontend helper that resolved the shared conversation from a task list.
- Updated user docs to describe `# all` as a stable Team-level conversation target.

## Validation

- `cargo test -p agenthub team_shared_thread_api_ -- --nocapture`
- `cargo test -p agenthub actor_mailbox_service_prefers_shared_thread_with_latest_message_when_duplicates_exist -- --nocapture`
- `cd web && npx vitest run src/pages/team/use_team_conversation_actions.test.tsx src/pages/team/page_helpers.test.ts src/pages/team_page.smoke.test.tsx`

## Follow-up

- Verify the dedicated shared-thread endpoints and canonical duplicate recovery behavior on
  deployed `agenthub.hawkingrei.com`, then record the run IDs and notes here.
