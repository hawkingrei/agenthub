# Agent Loop ACP Activity Filter

## Summary

Tightened `agent_loop` idle rearm semantics so the watchdog tracks ACP silence only, matching the
phase-1 contract.

## Change

- Added a focused `is_agent_loop_activity_output(...)` helper in `src/agent/manager.rs`.
- The loop controller now rearms only when the session receives non-loop `OutputStream::Acp`
  activity.
- `system`, `stdout`, and `stderr` events no longer directly reset the idle deadline for ACP
  watchdog prompts.
- A `broadcast::RecvError::Lagged(_)` still rearms conservatively because receiver overflow can
  hide real ACP activity even when noisy non-ACP output contributed to the lag.

## Why

The original controller reset its deadline on any output for the same session except its own
synthetic loop prompt. That meant unrelated runtime noise could postpone loop follow-up prompts even
though the feature is documented as watching ACP silence only.

## Validation

- `cargo test -q agent_loop_activity_counts_non_loop_acp_output_only --lib`
- `cargo test -q agent_loop_rearm_requires_same_session_and_real_acp_activity --lib`
