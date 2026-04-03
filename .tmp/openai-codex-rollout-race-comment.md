I dug into the current `codex-core` implementation and built a local fix for this race.

What appears to be happening:

- `Session::persist_rollout_items(...)` clones the rollout recorder handle under `services.rollout` and then performs the enqueue outside that mutex.
- `Session::shutdown(...)` later takes the shared recorder out of `services.rollout` and calls `RolloutRecorder::shutdown()`.
- `RolloutRecorder` clones share the writer `mpsc::Sender`, but they do not share an explicit shutdown lifecycle flag.

That lets a stale recorder clone enqueue after another clone has already started shutdown and closed the writer channel, which then surfaces as:

```text
failed to record rollout items: failed to queue rollout items: channel closed
```

I do not think this is only logging noise. The real risk is that tail rollout items can be dropped during normal session teardown, which can affect resume/history fidelity.

I have a local patch that fixes this by making recorder clones share shutdown state:

- add a shared `shutdown_started` flag across `RolloutRecorder` clones
- flip it before closing the writer task in `shutdown()`
- make late `record_items()` / `persist()` / `flush()` calls observe shutdown and return cleanly instead of racing a closed channel

I also added a focused reproducer at the recorder layer:

1. create a `RolloutRecorder`
2. clone it
3. call `shutdown()` on the original
4. call `record_items()` / `persist()` / `flush()` on the stale clone

That is enough to deterministically model the race without needing a full UI/session repro.

For reference, I currently carry the patch here while validating it downstream:
- branch: `hawkingrei/codex:agenthub/patches`
- patch commit: `18eaa6b8cdefd89a7a8ad8a0e1b0791fc33267bf`

If this direction matches what you want upstream, I can reshape the patch to whatever structure/style you prefer.
