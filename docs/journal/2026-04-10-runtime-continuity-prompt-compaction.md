# Runtime Continuity Prompt Compaction

## Summary

- slimmed the ACP runtime context block so continuity data stays pointer-first instead of embedding raw `history_window` JSON into prompt text
- kept continuity summary text in the prompt, but moved deeper history detail behind persisted continuity artifacts and replay state
- added a regression test to keep the runtime context block compact in future edits

## What Changed

- `crates/agenthub-acp/src/actor_runtime_skill.rs`
  - removed inline `continuity_history_window_json` from the runtime context block
  - replaced it with a stable `continuity_detail_policy` line that tells the runtime to treat deeper continuity detail as out-of-band state
  - added a test that asserts the block stays pointer-first and does not leak artifact paths or raw continuity JSON into prompt text

## Why

- The previous runtime context block still expanded bounded continuity history into inline JSON, which works against the ongoing prompt-tail slimming direction.
- Continuity storage and replay already exist outside the prompt surface, so the prompt only needs the compact summary plus a stable policy about where deeper detail lives.
- Keeping the continuity block narrower reduces unnecessary prompt churn while preserving the run identity and resume hint that the current turn actually needs.

## Validation

- `cargo test -p agenthub-acp actor_runtime_context_block`
