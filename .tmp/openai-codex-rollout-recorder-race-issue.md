## What issue are you seeing?

During normal session shutdown, Codex can emit:

```text
failed to record rollout items: failed to queue rollout items: channel closed
```

This appears to be more than a noisy shutdown log.

From reading the current `codex-core` implementation, the race is:

- `Session::persist_rollout_items(...)` clones the rollout recorder handle under the shared `services.rollout` mutex and then performs the enqueue outside that mutex.
- `Session::shutdown(...)` later takes the shared recorder out of `services.rollout` and calls `RolloutRecorder::shutdown()`.
- `RolloutRecorder` clones share the writer `mpsc::Sender`, but do not share an explicit shutdown lifecycle flag.

That means a stale recorder clone can still attempt to enqueue after another clone has already started shutdown and closed the writer channel, which then surfaces as `channel closed`.

The effect is not just logging noise: the tail rollout write for a session can be dropped during normal teardown, which can affect resume/history fidelity.

Relevant code paths in the pinned source tree:

- `codex-rs/core/src/codex.rs`
  - `persist_rollout_items(...)`
  - `shutdown(...)`
- `codex-rs/core/src/rollout/recorder.rs`
  - `record_items(...)`
  - `persist(...)`
  - `flush(...)`
  - `shutdown(...)`

## What steps can reproduce the bug?

A deterministic reproducer exists at the `RolloutRecorder` level and does not require UI interaction.

Minimal reproduction logic:

1. Create a `RolloutRecorder`.
2. Clone it so there is a stale clone.
3. Call `shutdown()` on the original recorder.
4. After shutdown starts, call `record_items()` / `persist()` / `flush()` on the stale clone.

Expected old behavior before a fix:
- the stale clone can race into the closed writer channel and return the same class of error as above.

In code, the reproducer is essentially:

```rust
let recorder = RolloutRecorder::new(path).await?;
let stale = recorder.clone();
recorder.shutdown().await?;
stale.record_items(vec![item]).await?;
stale.persist().await?;
stale.flush().await?;
```

In a real session, the same race shows up when teardown begins while late rollout items are still being appended.

## What is the expected behavior?

Normal session shutdown should not fail tail rollout persistence because of stale recorder clones racing a closed writer channel.

More specifically:

- once shutdown begins, late recorder clones should observe shutdown state and return cleanly
- rollout shutdown should be deterministic
- normal teardown should not surface `failed to queue rollout items: channel closed`
- resume/history/replay should not lose the last rollout items because of this race

## Additional information

I have a local patch that fixes this by giving `RolloutRecorder` clones a shared shutdown state:

- add a shared `shutdown_started` flag across recorder clones
- set it before closing the writer task in `shutdown()`
- make late `record_items()` / `persist()` / `flush()` calls treat shutdown as a clean no-op instead of racing the closed channel

This is currently carried in an AgentHub-specific Codex patch branch while waiting for an upstream fix.
