# ACP Permission Deny Prompt Drain

## Summary

Permission deny and timeout recovery no longer abandons the active ACP prompt after a fixed
post-cancel timeout. AgentHub still sends ACP cancel after a denied permission, but it now waits
for the provider prompt request to finish so provider-side follow-up events are not dropped by
AgentHub's prompt future.

## Background

The previous recovery path emitted a terminal failed `tool_call_update`, notified the ACP provider
with cancel, then waited five seconds for the prompt request to complete. If the provider did not
return in that window, AgentHub logged `acp prompt abandoned after permission denial timeout` and
marked the prompt complete locally.

That avoided a stuck active prompt, but it also meant AgentHub intentionally stopped waiting for
the provider-side prompt response. The safer behavior is to prefer draining the provider request
over local progress when the two conflict.

## Scope

- Remove the fixed post-deny prompt drain timeout.
- Keep the existing cancel notification on permission deny or timeout.
- Keep the active prompt occupied until the provider prompt request returns or fails.
- Preserve the existing terminal failed `tool_call_update` for the denied permission tool call.

## Key Decisions

- No-data-loss semantics take priority over local prompt throughput after permission denial.
- The provider remains responsible for completing the cancelled prompt. AgentHub does not invent a
  local completion when the provider is still running.
- Follow-up ordinary prompts may continue to queue behind the active prompt if the provider does
  not finish cancellation promptly.

## Validation

```bash
cargo fmt --all --check
cargo test -p agenthub-acp permission_den -- --nocapture
cargo test -p agenthub-acp denied_permission_tool_update_clears_diagnostics_pending_tool_call -- --nocapture
```

## Follow-Ups

- Run a real Codex ACP permission timeout after deployment and confirm the prompt drains without
  the old abandoned-message event.
