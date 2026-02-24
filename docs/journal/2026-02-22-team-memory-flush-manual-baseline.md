# Team Memory Flush Manual Baseline

## Background

The Team context architecture and memory-flush spec define an explicit flush lifecycle (`memory_flush_*`) and checkpointed incremental persistence.
Before this change, runtime had no executable flush path (only continuity snapshot updates at step completion).

## Scope

This change lands a manual flush baseline (no auto-trigger yet):

- backend API: `POST /api/teams/runs/:run_id/context/flush`;
- manager flush executor with checkpointed incremental event ingestion;
- file-backed flush artifact persistence under member workspace `.cache/context/run/<run_id>/...`;
- lifecycle events in `team_run_events`:
  - `memory_flush_started`
  - `memory_flush_persisted`
  - `memory_flush_noop`
  - `memory_flush_failed`

## Key Decisions

1. Trigger model for this phase
- API accepts `trigger` (`manual` / `soft_threshold` / `hard_error`) but this phase wires only manual entry.
- automatic soft/hard runtime trigger integration remains a follow-up.

2. Session resolution
- request may pass explicit `session_id`.
- when omitted, manager resolves latest non-empty `team_steps.remote_task_id` for `(run_id, member_id)`.
- if no session can be resolved, flush returns `failed` with `reason=session_mapping_missing` and emits `memory_flush_failed`.

3. Incremental checkpoint
- new table: `team_context_flush_checkpoint`
- key: `(run_id, member_id, session_id)`
- fields: `team_id`, `last_event_id`, `updated_at`
- flush reads `(last_event_id, +inf]` from `agent_events` and advances checkpoint only on persisted success.

4. Artifact indexing and storage
- flush artifacts reuse `team_context_artifacts` with `artifact_kind=memory_flush`.
- payload includes event range, summary text, and bounded observations.
- file path pattern:
  - `<member_workdir>/.cache/context/run/<run_id>/artifact-<seq>-memory_flush.json`

5. Safety and fallback
- write failures or missing member workdir do not fail run state machine.
- failure is surfaced by lifecycle event and response status (`failed`), keeping orchestration path non-blocking.

## API Contract

Endpoint:
- `POST /api/teams/runs/:run_id/context/flush`

Request:
- `member_id` (required)
- `session_id` (optional)
- `trigger` (optional, default `manual`)
- `max_events` (optional, clamped)

Response fields:
- `status` (`persisted` | `noop` | `failed`)
- `run_id`, `team_id`, `member_id`, `session_id`, `trigger`
- `reason` (when `noop`/`failed`)
- `artifact_pointer` (when persisted)
- `event_id_from`, `event_id_to`, `flushed_events`

## Schema Changes

`src/db.rs` adds:

- `team_context_flush_checkpoint`
- index `idx_team_context_flush_checkpoint_run_member`

`team_context_artifacts` is reused for flush artifact index rows.

## Validation

Executed targeted tests:

- `cargo test flush_run_context_`
- `cargo test team_runs_api_supports_manual_context_flush`
- `cargo test team_runs_api_rejects_invalid_context_flush_trigger`
- `cargo test teams_api_delete_team_cascades_related_run_data`
- `cargo test continuity`

New/updated coverage:

- `team::manager::tests::flush_run_context_persists_artifact_and_then_noops_with_checkpoint`
- `team::manager::tests::flush_run_context_fails_when_session_mapping_missing`
- `api::teams::tests::team_runs_api_supports_manual_context_flush`
- `api::teams::tests::team_runs_api_rejects_invalid_context_flush_trigger`
- cascade test now validates cleanup for `team_context_artifacts` and `team_context_flush_checkpoint`

## Follow-ups

Still pending:

- auto-trigger integration from runtime soft-threshold / hard-error signals;
- compaction-path hook wiring (`pre-compaction flush`);
- retry/metrics/observability completeness against full spec.
