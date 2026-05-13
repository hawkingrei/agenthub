# ACP Permission Deny Prompt Drain

## Summary

Permission deny and timeout recovery no longer cancels or abandons the active ACP prompt after a
permission decision. AgentHub returns the concrete deny outcome through the ACP permission response
and waits for the provider prompt request to finish, so provider-side follow-up events are not
dropped by AgentHub's prompt future.

## Background

The previous recovery path emitted a terminal failed `tool_call_update`, notified the ACP provider
with session cancel, then waited five seconds for the prompt request to complete. If the provider
did not return in that window, AgentHub logged `acp prompt abandoned after permission denial
timeout` and marked the prompt complete locally.

That avoided a stuck active prompt, but it conflated a denied tool permission with cancellation of
the whole prompt turn and could stop waiting for provider-side completion. The safer behavior is to
return the deny outcome and keep draining the provider request.

## Scope

- Remove the fixed post-deny prompt drain timeout.
- Remove the automatic session cancel notification on permission deny or timeout.
- Keep the active prompt occupied until the provider prompt request returns or fails.
- Preserve the existing terminal failed `tool_call_update` for the denied permission tool call.

## Key Decisions

- No-data-loss semantics take priority over local prompt throughput after permission denial.
- Permission deny is communicated by the ACP permission response, not by session cancel.
- The provider remains responsible for completing the prompt after receiving the deny outcome.
  AgentHub does not invent a local completion when the provider is still running.
- Follow-up ordinary prompts may continue to queue behind the active prompt if the provider does
  not finish the denied prompt promptly.

## Validation

```bash
cargo fmt --all --check
cargo test -p agenthub-acp denied_permission_tool_update_clears_diagnostics_pending_tool_call -- --nocapture
```

## Follow-Ups

- Run a real Codex ACP permission timeout after deployment and confirm the prompt drains without
  the old cancel or abandoned-message event.
