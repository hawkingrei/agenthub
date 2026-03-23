## Summary

- removed the backend Team orchestrator worker from the default runtime path
- stopped implicit `task -> run` auto-dispatch during task creation
- added nullable `assigned_member_id` as the canonical task-owner field, with default `NULL`

## Why

The previous Team model still treated run/step orchestration as the active execution backbone.
That made task ownership blurry:

- tasks looked like planning wrappers around step dispatch
- creating a task implicitly created a run even when no explicit execution should start
- there was no durable task-level owner field

The updated direction is task-first:

- `task` is the primary collaboration and ownership unit
- `run`/`step` remain execution/debug artifacts
- backend should not guess or schedule ownership implicitly

## What Changed

### Runtime

- removed `TeamOrchestratorWorker` autostart from `AppState::init`
- removed the backend orchestrator module from the compiled Team module tree
- dropped the router test that existed only to validate the removed worker path
- kept `list_active_runs` as a test-only helper instead of shipping an unused production API

### Task model

- added nullable `assigned_member_id` to `TeamTaskRecord`
- added `assigned_member_id` to the `team_tasks` table schema
- added a DB migration so existing deployments gain the new nullable column
- task creation now persists `assigned_member_id = NULL`

### Task behavior

- `create_team_task` no longer auto-creates a linked run
- `TeamTaskDetailResponse.latest_run` stays `None` immediately after task creation unless a run is created explicitly later
- existing task list/get APIs now expose the nullable owner field

### Active docs / skills

- `docs/features/agents-teams.md`
- `docs/features/teams-collaboration-playbook.md`
- `skills/team/team-task-lifecycle.SKILL.md`

These now describe:

- task-first ownership
- no backend step-orchestrator worker in the default path
- `assigned_member_id` remaining empty until explicitly assigned

## Validation

- `cargo test -p agenthub-db init_db_adds_assigned_member_id_to_existing_team_tasks_table -- --nocapture`
- `cargo test -p agenthub list_active_runs_returns_non_terminal_runs_only -- --nocapture`
- `cargo test -p agenthub cancel_active_runs_on_startup_reopens_linked_tasks -- --nocapture`
- `cargo test -p agenthub team_task_api_creates_lists_and_redacts_context -- --nocapture`
- `cargo test -p agenthub team_task_messages_api_forwards_team_chat_to_active_run_mailbox -- --nocapture`
- `cargo test -p agenthub teams_router_http_contract -- --nocapture`
- `cargo fmt --all --check`
- `cargo clippy --locked -p agenthub --all-targets -- -D warnings`
- `cargo clippy --locked -p agenthub-db --all-targets -- -D warnings`
