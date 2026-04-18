# Team Prompt Tail State Paths

## Summary

- advanced the Team prompt-tail slimming follow-up by moving continuity recovery detail into the
  workspace-backed runtime state index instead of replaying it inline on every ACP turn;
- kept the ACP runtime context block pointer-first: it now points at `.cache/context/state.md` and
  any persisted continuity artifact path, instead of embedding `continuity_summary` prose;
- preserved continuity mode and source run identity in the prompt so recovery still has enough
  routing context without carrying a replay-sized tail.

## Implementation Notes

- `src/team/manager.rs`
  - after each continuity update, rewrite workspace `.cache/context/state.md` with a compact Team
    runtime state snapshot;
  - include current run id, continuity mode, source run id, short continuity summary, and optional
    continuity artifact path in the file-backed snapshot.
- `crates/agenthub-acp/src/actor_runtime_skill.rs`
  - changed the injected ACP runtime context block to emit:
    - `continuity_mode`
    - `continuity_source_run_id`
    - `continuity_state_path: .cache/context/state.md`
    - `continuity_artifact_path` when available
  - removed inline `continuity_summary` from the prompt block;
  - accepted both legacy string and current object-shaped `artifact_pointer` payloads so prompt
    rendering stays compatible with older continuity records.
- `crates/agenthub-acp/src/lib.rs`
  - updated prompt-prefix regression coverage for the new pointer-first runtime context contract.

## Validation

- `cargo test complete_step_offloads_large_output_to_workspace_context_artifact -- --nocapture`
- `cargo test -p agenthub-acp actor_runtime_context_block_keeps_continuity_pointer_first -- --nocapture`
- `cargo test -p agenthub-acp prompt_prefix_blocks_append_runtime_context_after_skills -- --nocapture`
- `cargo fmt`

## Notes

- This does not close the broader TODO item yet; it only moves one dynamic continuity slice from
  prompt prose into filesystem-backed runtime state.
- Review follow-up tightened the runtime path:
  - `state.md` snapshot writes are now async and best-effort, so transient filesystem issues do
    not roll back `complete_step`;
  - manager-side snapshot rendering now accepts both legacy string and current object-shaped
    `artifact_pointer` payloads, matching the ACP runtime block compatibility path.
