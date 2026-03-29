# Public Team Task-Create Contract

## Summary

- removed public `POST /api/teams/:id/tasks` from the Team HTTP router
- kept canonical Team task creation on leader/runtime-only paths (`agenthub actor team-task-create`
  and internal gRPC control)
- aligned tests and docs with the existing task-first Team UI contract

## Why

The Team workbench had already moved to a task-first model:

- humans request work in `Conversation` (`# all`)
- leader/runtime materialize canonical Kanban tasks
- human/public HTTP clients can read task state, but they do not own canonical task writes

That contract was already reflected in the UI and stable feature docs, but the public router still
exposed `POST /api/teams/:id/tasks`. Keeping the route around created an unnecessary second entry
point and let external callers keep treating human task creation as a supported public workflow.

## What Changed

- removed the public Team router `POST /api/teams/:id/tasks` binding
- removed the unused frontend `api.createTeamTask(...)` helper
- kept a test-only fixture helper so API tests can still seed canonical Team tasks without
  reintroducing the removed public route into the product contract
- updated router coverage so the path now returns `405 Method Not Allowed` on the public surface
- updated stable docs and userdocs to say the normal Team UI/public HTTP surface does not expose
  direct canonical task creation

## Validation

- `cargo test -p agenthub teams_router_http_contract -- --nocapture`
- `cargo test -p agenthub team_task_api_lists_gets_and_redacts_context -- --nocapture`
- `cargo test -p agenthub team_task_api_enforces_team_owner_access_for_existing_tasks -- --nocapture`
- `cd web && npm run lint -- src/api.ts`
- `npm --prefix userdocs run build`
- `cargo fmt --all`
- `git diff --check`
