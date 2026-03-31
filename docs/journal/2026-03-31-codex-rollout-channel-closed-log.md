# Codex rollout channel-closed log suppression

## Summary

AgentHub-managed Codex ACP sessions could emit this log during shutdown:

```text
ERROR codex_core::codex: failed to record rollout items: failed to queue rollout items: channel closed
```

The underlying `codex-core` behavior is a shutdown race: a turn can still try to append rollout items after the rollout writer channel has already been torn down. In this case the send fails because the receiver is closed, but the session is already shutting down and no user-visible recovery action exists.

For the AgentHub adapter this is shutdown noise, not a signal that mailbox persistence or ACP transport failed.

## Implementation

- Keep the upstream behavior untouched.
- Suppress only this exact benign shutdown log in `agenthub-codex-acp` log formatting.
- Leave all other `codex_core::codex` errors visible.

The suppression matches:

- target: `codex_core::codex`
- level: `ERROR`
- message prefix: `failed to record rollout items: failed to queue rollout items: channel closed`

## Validation

- Focused unit tests in `agenthub-codex-acp/src/lib.rs` cover matching and non-matching cases.
- Merge follow-up should capture PR/push CI run IDs before the TODO is closed.
