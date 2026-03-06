# 2026-03-06 Codex ACP Disable Implicit Responses WebSocket

## Context

`agenthub-codex-acp` currently pins codex git dependencies to `c34b30a3c128bb75fcec27ef838c93c99b92fc61`.

That upstream revision still enables responses websocket transport when all of the
following are true:

- the selected model provider reports `supports_websockets = true`
- either websocket feature flags are enabled or model metadata marks
  `prefer_websockets = true`

For local Codex configs that select models such as `gpt-5.4`, the model cache can
advertise `prefer_websockets = true` even when the user did not explicitly enable
responses websocket features. In AgentHub this surfaced as repeated runtime errors:

- `failed to connect to websocket: HTTP error: 426 Upgrade Required`
- follow-up stream disconnect / reconnect noise after websocket prewarm failed

## Decision

Apply a minimal local fix in `agenthub-codex-acp` instead of doing a full upstream
sync immediately.

During config load, if responses websocket features are not explicitly enabled,
normalize `config.model_provider.supports_websockets` to `false` before constructing
the Codex agent. This keeps explicit websocket opt-in behavior intact while blocking
the old upstream's implicit websocket path.

## Scope

- `agenthub-codex-acp/src/lib.rs`

## Implementation Notes

1. Add a small helper that treats only these features as websocket opt-in:
   - `Feature::ResponsesWebsockets`
   - `Feature::ResponsesWebsocketsV2`
2. After `Config::load_with_cli_overrides_and_harness_overrides(...)`, disable
   provider websocket support unless one of those features is enabled.
3. Add unit tests that cover:
   - implicit websocket disable path
   - explicit `ResponsesWebsockets` opt-in
   - explicit `ResponsesWebsocketsV2` opt-in

## Follow-up

- Prefer a future codex upstream sync to remove this local compatibility shim once
  the pinned revision includes the explicit-feature-only websocket gating.
- Validate an end-to-end Codex ACP turn with `gpt-5.4` and default local config to
  confirm the runtime stays HTTP-only and no longer emits websocket `426` errors.

## Validation

- `cargo +1.93.1 test -p agenthub-codex-acp --lib`
- result: `32 passed; 0 failed`
