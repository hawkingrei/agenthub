# Team Runtime Event Handle Naming

## Goal

Continue the Team execution-vocabulary cleanup on the Rust backend without widening scope into a
schema or API migration.

## Why

The compatibility rollout already made `runtime_handle_id` the preferred Team step field name in
Rust and web surfaces, but Team run events still emitted mixed payload names:

- `step_working` / `step_resumed` included both `runtime_handle_id` and legacy
  `remote_task_id`;
- `continuity_state_updated` still exposed `source_session_id` even when the value represented the
  Team step runtime handle.

That kept runtime/debug events semantically noisy and pulled execution telemetry away from the
canonical vocabulary in `docs/features/team-execution-vocabulary.md`.

## Change

- Added a shared helper in `src/team/manager.rs` for step runtime-handle event payloads.
- Added a shared helper for continuity-state event payloads.
- `step_working` and `step_resumed` now emit only `runtime_handle_id`.
- `continuity_state_updated` now emits `source_runtime_handle_id` instead of
  `source_session_id`.
- Kept the SQLite column name (`team_steps.remote_task_id`) and continuity-state record field
  (`source_session_id`) unchanged for compatibility in this change.

## Validation

Focused regression coverage should verify:

- `step_working` payload keeps `runtime_handle_id` and omits `remote_task_id`;
- `step_resumed` payload keeps `runtime_handle_id` and omits `remote_task_id`;
- `continuity_state_updated` payload keeps `source_runtime_handle_id` and omits
  `source_session_id`.

Recommended commands:

```bash
cargo test -q complete_step_offloads_large_output_to_workspace_context_artifact --lib
cargo test -q input_required_and_resume_transitions_update_run_and_emit_events --lib
```
