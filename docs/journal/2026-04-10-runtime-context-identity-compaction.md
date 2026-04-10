# Runtime Context Identity Compaction

## Summary

- further slimmed the ACP runtime context block by dropping fields that do not help the current actor choose the next action
- removed the `current_run_id: n/a` placeholder when no active run scope exists
- removed `continuity_source_session_id` from prompt text so provider continuity internals stay outside the actor-facing working set

## What Changed

- `crates/agenthub-acp/src/actor_runtime_skill.rs`
  - stopped emitting a synthetic `current_run_id: n/a` line when the runtime has no current run id
  - stopped emitting `continuity_source_session_id` in the continuity block
  - added focused regression coverage for both behaviors

## Why

- The runtime context block should bias toward fields that affect the actor's immediate execution decision.
- A missing run id does not need an explicit placeholder in prompt text; omission is cheaper and clearer.
- Provider session ids are useful for backend continuity bookkeeping, but they are not actionable for leader/worker behavior inside a prompt turn.

## Validation

- `cargo test -p agenthub-acp actor_runtime_context_block`
