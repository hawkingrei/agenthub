## Summary

Kept the canonical Team `# all` shared-thread discoverable across refreshes by adding an explicit
public task-list query flag and using it from the Team conversation loader.

## Why

The Team page resolves the current `# all` conversation from the fetched task list. The public
`GET /api/teams/:id/tasks` path reused the workspace-task default, which excludes
`bootstrap_kind=shared_thread`. After a refresh, the page could no longer see the existing shared
thread and the next send path created a new `all` task, making older conversation history appear to
vanish.

## Changes

- Added `include_shared_thread` to the public Team task list query.
- Updated Team page task refresh and shared conversation bootstrap to request shared-thread tasks.
- Added regression coverage to ensure the client reuses an existing shared thread instead of
  creating a duplicate.
- Removed the obsolete `TeamManager::list_tasks` wrapper so call sites must spell out
  `TeamTaskListQuery` semantics, including whether shared-thread tasks are included.

## Validation

- `cargo test -p agenthub team_task_list_api_can_include_shared_thread_when_requested -- --nocapture`
- `cd web && npx vitest run src/pages/team/use_team_conversation_actions.test.tsx`
