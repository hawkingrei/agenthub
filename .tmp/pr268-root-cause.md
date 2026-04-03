## Summary

- revert the adapter-side rollout log suppression from this branch
- backport the upstream `codex-core` rollout-recorder shutdown-race fix through AgentHub's dedicated Codex patch branch
- document the real root cause and keep a post-merge verification item for the runtime symptom

## Root Cause

This is not only a noisy log-level issue.

In the pinned Codex revision, `Session::persist_rollout_items(...)` cloned the rollout recorder handle under the shared mutex and then sent `AddItems` outside that lock, while `Session::shutdown(...)` later took the shared recorder and shut down the writer task.

`RolloutRecorder` clones only shared the `mpsc::Sender`, not an explicit shutdown lifecycle flag. That allowed a stale recorder clone to enqueue after another clone had already started shutdown and closed the writer channel, which surfaced as:

```text
failed to record rollout items: failed to queue rollout items: channel closed
```

The real risk is tail rollout persistence inconsistency during normal session teardown, which can affect resume/history fidelity. It is not just harmless logging noise.

## Fix

- move AgentHub's Codex maintenance line onto `hawkingrei/codex:agenthub/patches`
- pin `agenthub-codex-acp`'s Codex dependencies to that dedicated patch branch
- the current branch head contains the narrow upstream backport:
  - `RolloutRecorder` clones now share `shutdown_started`
  - `shutdown()` flips that shared state before closing the writer task
  - late `record_items()` / `persist()` / `flush()` calls from stale clones now observe shutdown and return cleanly instead of racing a closed channel
- remove the earlier adapter-only suppression direction from this branch so the PR reflects the actual fix

## Testing

- `git diff --check`
- attempted focused upstream validation:
  - `cargo +1.94.0 test -p codex-core stale_recorder_clone_ignores_late_writes_after_shutdown -- --nocapture`
  - blocked locally by `No space left on device` while compiling the upstream Codex workspace

## Notes

- upstream patch branch: `hawkingrei/codex:agenthub/patches`
- current pinned patch commit: `18eaa6b8cdefd89a7a8ad8a0e1b0791fc33267bf`
- follow-up merge verification remains tracked in `docs/todo.md`
