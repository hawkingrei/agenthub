## Summary

- expanded the Team Kanban actor CLI from a task status register into a usable inspect/query tool
- aligned `team-tasks` with run-scoped lookup by adding `--run-id` plus focused task filters
- added single-task inspect, append-only task notes, and context patch support without changing the public HTTP task mutation contract

## Why

The previous CLI could mutate task status, but routine leader workflows were still awkward:

- `team-members` and `inbox` accepted `--run-id`, while `team-tasks` did not
- task lookup mostly required listing everything and grepping locally
- task update semantics stopped at status plus assignment, so leaders still had to track richer task meaning outside the canonical task path
- there was no direct `show/get` path for a single task

That made the CLI feel closer to a narrow register than a real Kanban operator surface.

## What Changed

### Query and inspect

- `agenthub actor team-tasks` now supports:
  - `--run-id`
  - `--task-id`
  - `--assigned-member-id`
  - `--topic`
  - documented `--json`
- `--run-id` is treated as a Team scope selector by resolving the Team from the run and then applying task filters inside that Team; it does not redefine `run_id` as task identity
- added `agenthub actor team-task-show --task-id ...`
- added alias `agenthub actor team-task-get --task-id ...`
- single-task inspect returns:
  - task record
  - task conversation metadata
  - latest linked run, if any
  - recent task conversation messages

### Task mutation semantics

- `agenthub actor team-task-update` now supports repeated `--task-id` for small batch updates
- added context patch inputs:
  - `--context-json`
  - `--context-json-file`
  - `--context-merge-json`
  - `--context-merge-json-file`
- `context_json` replaces the stored task context after redaction
- `context_merge_json` deep-merges into the stored task context after redaction
- `assigned_member_id` help text now states the current CLI contract explicitly: it represents the current execution owner

### Append-only notes

- added `agenthub actor team-task-note`
- supported note kinds:
  - `comment`
  - `decision`
  - `result`
- note payloads are appended to the task conversation instead of overloading task row mutation

### Internal control and output

- extended internal gRPC task controls to support:
  - filtered Team task listing
  - single-task fetch
  - task note append
  - context replace / merge on task update
- refreshed actor CLI help so the new flags and commands are discoverable
- marked `team-task-show` and `team-task-note` as TOON-preferred outputs to keep human-readable responses stable

## Validation

- `cargo check --locked`
- `cargo test parse_team_task -- --nocapture`
- `cargo test task_context_patches_support_merge_and_replace -- --nocapture`
- `cargo test list_tasks_with_query_filters_by_run_topic_and_owner -- --nocapture`
- `cargo test internal_grpc_team_context_and_task_controls_are_wire_compatible -- --nocapture`

## Follow-ups

- repo / issue / PR structured filters are still out of scope for this change
- if ownership semantics need to split into `implementation owner` vs `next-action owner`, that should be a schema/contract change instead of more CLI wording alone
