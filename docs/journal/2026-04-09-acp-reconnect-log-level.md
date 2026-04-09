# ACP Reconnect Log Level

## Summary

- demoted handled `StreamError` turn logs in `agenthub-codex-acp` from `error!` to `warn!`
- kept `EventMsg::Error` as `error!` because it still terminates the active turn

## Why

`EventMsg::StreamError` is emitted for recoverable upstream stream interruptions such as websocket reconnect attempts. AgentHub logs these events as handled and Codex retries the turn automatically, so `error!` overstated the severity and made normal reconnect behavior look like a hard failure in production logs.

## Validation

- `cargo check -p agenthub-codex-acp`
