# Team Prompt Tail And Execution Run Wording

## Summary

- continued Team prompt-tail slimming by moving continuity detail out of inline runtime prompt text and into a run-scoped filesystem note
- tightened Team runtime and web wording so concrete execution partitions are labeled as `execution run` instead of a generic `run` surface label

## Prompt Tail Follow-up

- `src/team/manager.rs` now writes a compact continuity note to
  `.cache/context/run/<source_run_id>/continuity.md`
- `.cache/context/state.md` stays as the compact index and now points to:
  - `current_execution_run_id`
  - `continuity_source_execution_run_id`
  - `continuity_note_path`
  - `continuity_artifact_path` when present
- the ACP runtime context block now stays pointer-first and exposes
  `continuity_note_path` instead of relying on inline continuity summary prose

## Execution Vocabulary Follow-up

- Team web tabs and run-focused controls now use `Execution Runs` / `Execution Run`
  wording in the primary UI shell
- active-run actions and task-detail affordances now say
  `Resume Execution Run`, `Restart Execution Run`, and `Open Execution Run`
- run-required empty states now point users to the `Execution Runs` tab explicitly

## Validation

- `cargo test complete_step_offloads_large_output_to_workspace_context_artifact -- --nocapture`
- `cargo test complete_step_keeps_success_when_runtime_state_snapshot_write_fails -- --nocapture`
- `cargo test -p agenthub-acp actor_runtime_context_block_keeps_continuity_pointer_first -- --nocapture`
- `cargo test -p agenthub-acp prompt_prefix_blocks_append_runtime_context_after_skills -- --nocapture`
- `cargo fmt`
- `cd web && npm run test -- vite.config.test.ts src/pages/team_panels.test.tsx src/pages/team_page.smoke.test.tsx src/pages/team/team_debug_panels.test.tsx src/pages/team/team_workspace_header.test.tsx`
- `cd web && npm run build`
