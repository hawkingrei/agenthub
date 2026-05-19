# Team Task Priority, Notes, And Governance

## Summary

Expanded the Team task contract so canonical Kanban tasks now carry explicit priority, expose a
separate note journal, and enforce stronger coordinator-side governance.

## What Changed

- Added `task.priority` with stable values `critical`, `high`, `medium`, and `low`.
- Extended task list APIs and internal actor controls so tasks can be filtered by priority.
- Promoted task notes into a first-class journal view backed by persisted `task_note` messages.
- Required canonical task creation through internal actor controls to include an
  `assigned_member_id`.
- Required deliberate task status transitions to include a same-update note (`comment`,
  `decision`, or `result`).
- Updated Team task board UI to:
  - show priority badges
  - sort higher-priority work first inside status lanes
  - filter the board by priority
  - render a dedicated notes/journal section in task detail
- Tightened coordinator/worker default prompts so agents understand the new priority and note
  expectations.

## Files

- Domain and persistence:
  - `crates/agenthub-team-domain/src/lib.rs`
  - `crates/agenthub-db/src/lib.rs`
  - `src/team/manager.rs`
  - `src/team/manager/codec.rs`
- Internal actor contract:
  - `src/internal/proto/agenthub.internal.v1.rs`
  - `src/internal/client/control.rs`
  - `src/internal/client/mod.rs`
  - `src/internal/service/helpers.rs`
  - `src/internal/service/rpc.rs`
  - `src/actor_cli.rs`
  - `src/actor_cli/parse.rs`
  - `src/actor_cli/execute.rs`
  - `src/actor_cli/help.rs`
- Public/task UI surfaces:
  - `src/api/teams.rs`
  - `web/src/api.ts`
  - `web/src/pages/team/use_team_task_workspace_data.ts`
  - `web/src/pages/team/team_workspace_context.tsx`
  - `web/src/pages/team/TeamTasksContainer.tsx`
  - `web/src/pages/team_page.tsx`
  - `web/src/pages/team_tasks_panel.tsx`
- Prompts:
  - `crates/agenthub-team-prompts/prompts/default_team_coordinator_prompt.txt`
  - `crates/agenthub-team-prompts/prompts/default_team_worker_prompt.txt`

## Validation

Executed:

```bash
TMPDIR=/home/hawkingrei/devel/opensource/agenthub/.tmp cargo check -p agenthub --tests
TMPDIR=/home/hawkingrei/devel/opensource/agenthub/.tmp cargo test -p agenthub-db init_db_adds_priority_to_existing_team_tasks_table -- --nocapture
npm exec tsc -- --noEmit
npm exec vitest -- run src/pages/team/use_team_task_workspace_data.test.tsx src/pages/team_panels.test.tsx
```

Notes:

- Focused web tests passed after updating the new `selectedTaskDetail` hook contract.
- Rust `cargo check` passed with the task priority/note protocol changes in place.
- The heavier `agenthub` integration tests remain expensive to build in this workspace because they
  pull the full `lancedb` test graph; the compile path is valid, but broader test selection should
  stay focused when iterating on Team task flows.
