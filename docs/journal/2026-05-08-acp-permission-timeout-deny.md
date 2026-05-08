# ACP Permission Timeout Deny Handling

## Summary

ACP permission review timeout and response-channel failure now produce a concrete reject decision
instead of returning ACP `cancelled`. Active agent sessions also expose permission wait state as
`waiting_permission`, and only return to `running` after every pending permission for that session
has reached a terminal state.

## Background

Codex ACP permission options are explicit choices: allow once, allow persistently, reject once, or
reject persistently. ACP `cancelled` is reserved for prompt-turn cancellation, not for ordinary
permission review timeout. Returning `cancelled` for review failure can leave the provider-side
turn without the concrete deny result that the tool call expects.

## Scope

- `session/request_permission` emits a `waiting_permission` run status when review starts.
- Timeout and response-channel-close paths select `RejectOnce` first, then `RejectAlways`.
- `cancelled` remains only as a final fallback when the provider supplied no reject option.
- `acp_permission_requests.selected_option_id` is populated for timeout deny outcomes.
- `agent_sessions.status` remains `waiting_permission` while any permission in that session is
  still pending, then restores to `running`.
- Web live-output status mapping treats `waiting_permission` as an active agent state.

## Key Decisions

- The default failure policy is deny, not cancel. This matches the product requirement that every
  permission request must be processed and keeps timeout behavior aligned with Codex option
  semantics.
- Session status restoration is guarded by a pending-permission check so concurrent permission
  requests cannot hide each other by restoring `running` too early.
- Agent-level active/inactive classification should not treat permission wait as stopped, because
  the ACP process is still alive and waiting on a controlled decision point.

## Validation

```bash
cargo test -p agenthub-acp permission_
npm exec vitest -- run src/app_live_output.test.ts
cargo fmt --all --check
git diff --check
```

## Follow-Ups

- Re-check PR CI and Codecov after the latest push finishes uploading Rust/Web reports.
- After merge and restart, validate a real Codex ACP permission timeout on
  `agenthub.hawkingrei.com` and confirm the member status returns from `waiting_permission` to
  `running` without leaving a tool call permanently in progress.
