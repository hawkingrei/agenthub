# Team Context Artifact Pointer Baseline

## Background

Team continuity (Track 1/3) previously stored bounded snapshots only in SQLite (`team_member_continuity_state`).
Large step outputs were truncated into inline JSON excerpts, but the full redacted payload had no file-backed persistence path.

## Scope

This change lands a baseline implementation for file-backed Team context artifacts:

- add artifact metadata persistence (`team_context_artifacts`);
- offload oversized continuity payloads into member workspace-local `.cache/context/run/<run_id>/...` files;
- keep continuity prompt payload pointer-first (bounded excerpt + artifact pointer);
- preserve non-fatal fallback behavior when offload is unavailable.

## Key Decisions

1. Trigger policy
- Offload is triggered when redacted continuity output exceeds the inline history budget (`CONTINUITY_MAX_HISTORY_CHARS`, currently `4096`).

2. Workspace ownership
- Artifact root is resolved from the member agent's `agents.workdir`.
- Files are written under:
  - `<member_agent_workdir>/.cache/context/run/<run_id>/artifact-<seq>-continuity-output.json`

3. Metadata index
- New table: `team_context_artifacts`
- Persisted fields include `team_id`, `run_id`, `member_id`, `session_id`, `artifact_seq`, `artifact_kind`, `artifact_path`, `artifact_size_bytes`, `content_checksum`, `created_at`.
- Added indexes for `(run_id, member_id, created_at)` and unique `(run_id, artifact_seq)`.

4. Prompt/continuity payload shape
- `history_window` keeps bounded `output_excerpt` and now includes optional `artifact_pointer` when offload succeeds.
- `continuity_state_updated` event now includes:
  - `artifact_offload_status` (`inline` or `persisted`)
  - optional `artifact_pointer`
  - optional `artifact_offload_reason` (`agent_workdir_missing` or `artifact_write_failed`)

5. Failure behavior
- Missing agent workdir or filesystem write failure does not fail step completion.
- Runtime falls back to inline continuity snapshot and logs warning diagnostics.

## Security And Redaction

- Existing sensitive-key redaction is reused before excerpt generation and before artifact write.
- Persisted artifact stores redacted payload, not raw secret-bearing content.

## Data Lifecycle Notes

- Team deletion now removes `team_context_artifacts` rows together with other Team-owned run data.
- Artifact files remain workspace-local under `.cache/` and are treated as local runtime artifacts.

## Validation

Executed targeted Rust tests:

- `cargo test continuity`
- `cargo test complete_step_offloads_large_output_to_workspace_context_artifact`
- `cargo test init_db_creates_schema_and_enforces_foreign_keys`
- `cargo test team_main_task_api_creates_lists_and_redacts_context`
- `cargo test step_lifecycle_transitions_persist_and_emit_events`

New regression test:

- `team::manager::tests::complete_step_offloads_large_output_to_workspace_context_artifact`
  - verifies offload artifact creation,
  - pointer metadata persistence,
  - `continuity_state_updated` payload enrichment,
  - redaction marker presence in persisted artifact.

## Follow-ups

Still pending and tracked separately:

- pre-compaction memory flush lifecycle (`memory_flush_*` events);
- flush checkpoint/idempotency (`team_context_flush_checkpoint`);
- full workspace-isolation enforcement and cross-workspace mediation validation in end-to-end runs.
