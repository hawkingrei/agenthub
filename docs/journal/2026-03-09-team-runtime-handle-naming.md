# Summary

Recorded the compatibility-safe follow-up for Team step runtime-handle naming. The current field name `remote_task_id` is now misleading because the ACP-backed runtime stores member agent session ids in that slot.

## Current State

The Team runtime originally treated `remote_task_id` as a generic external-executor handle. The current implementation uses it as the step-level member session handle:

- the orchestrator starts a member agent session;
- the returned `session_id` is written into `team_steps.remote_task_id`;
- later reconciliation and step transitions use that value as the running session handle.

After rolling back the run-scoped eager-startup experiment, the mismatch is still clear:

- `team_steps.remote_task_id` only represents the runtime handle bound to the actively dispatched step;
- Team member session ownership is being moved away from run state;
- the legacy field name still does not describe what the value actually means.

## Why Not Rename Immediately

`remote_task_id` currently crosses multiple compatibility boundaries:

- SQLite schema: `team_steps.remote_task_id`
- Rust domain types: `TeamStepRecord.remote_task_id`
- REST payloads: `StartTeamRunStepRequest.remote_task_id`, `TeamStepRecord`
- internal proto payloads: `TransitionStepRequest/Response.remote_task_id`
- tests and OpenAPI schema

Renaming it in one change would be a compatibility change, not a local refactor.

## Recommended Direction

Treat the field as a **runtime handle** abstraction instead of a literal "remote task id".

Preferred target names:

1. `runtime_handle_id`
   - best if Team steps may later run on non-session-backed executors;
   - keeps the abstraction generic.
2. `member_session_id`
   - best if Team step execution is expected to stay ACP-session-backed long term;
   - most explicit for the current implementation.

At the moment, `runtime_handle_id` is the safer long-term direction because the Team roadmap still includes remote peers/nodes and alternative execution topologies.

## Migration Shape

Recommended compatibility-safe rollout:

1. document the current semantics in domain/API code comments;
2. introduce the clearer replacement field alongside the legacy one in domain/API/proto layers;
3. dual-read / dual-write during the transition;
4. update UI/tests/docs to the new field name;
5. only remove `remote_task_id` after explicit compatibility cleanup.

## Validation

When the rename work is implemented, validate at least:

- Team step lifecycle APIs still start/reconcile/complete steps correctly;
- Team member-scoped runtime lifecycle stays distinct from step-level runtime handles;
- OpenAPI and internal proto payloads remain compatible during the transition window;
- `/api/teams/:id/runs/:run_id/snapshot` and router tests preserve stable behavior.
