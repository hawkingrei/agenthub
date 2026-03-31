# Codex rollout recorder shutdown race fix

## Summary

AgentHub-managed `agenthub-codex-acp` sessions could surface:

```text
failed to record rollout items: failed to queue rollout items: channel closed
```

This was not just a noisy log level problem. The underlying `codex-core`
`RolloutRecorder` clones did not share shutdown state, so a stale clone could
still try to enqueue rollout items after another clone had already started
shutdown and closed the writer channel.

## Root cause

In the pinned upstream revision:

- `Session::persist_rollout_items(...)` cloned the recorder handle under
  `services.rollout` and sent `RolloutCmd::AddItems` outside the mutex.
- `Session::shutdown(...)` later took the shared recorder out of
  `services.rollout` and called `RolloutRecorder::shutdown()`.
- `RolloutRecorder` clones only shared the `mpsc::Sender`, not an explicit
  lifecycle flag.

That meant late writers observed shutdown indirectly through a closed channel,
which surfaced as an error and could drop the tail rollout write during normal
session teardown.

## Fix

Backport a narrow `codex-core` fix on top of the pinned Codex revision via
`hawkingrei/codex@agenthub-rollout-recorder-shutdown-race-v1`
(`18eaa6b8cdefd89a7a8ad8a0e1b0791fc33267bf`):

- `RolloutRecorder` clones now share `shutdown_started`.
- `shutdown()` sets that flag before sending the writer shutdown command.
- Late `record_items()` / `persist()` / `flush()` calls from stale clones now
  observe shutdown and return cleanly instead of racing into `channel closed`.

This keeps shutdown semantics deterministic without broadening the adapter-side
logging layer.

## Validation

- upstream backport branch: `hawkingrei/codex:fix/rollout-recorder-shutdown-race`
- AgentHub follow-up should record PR/push CI run IDs before the TODO is closed
