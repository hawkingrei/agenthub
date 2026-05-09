# Team ACP Input Submission Diagnostics

## Summary

Team member ACP input now leaves a visible, redacted ACP conversation event when AgentHub accepts
the user's input record but cannot submit the prompt command to the ACP provider.

## Background

The Team member ACP panel already refreshes after successful sends. The weak path was a backend
submission failure after the user message had been persisted: the HTTP request returned an error,
but the ACP conversation could still look like nothing happened until a later manual refresh or
diagnostic check.

## Scope

- Backend `send_input` still persists and broadcasts the user message before submitting the prompt
  command.
- If ACP prompt submission fails after that persistence point, AgentHub now appends a synthetic
  `agent_message` ACP event that tells the operator to inspect redacted provider diagnostics.
- The synthetic event does not include the prompt body, tool arguments, tool output, environment
  values, or provider tokens.
- Team member ACP input failure handling now refreshes the selected member events once, so the
  persisted input and failure marker can become visible without waiting for SSE fallback timing.

## Key Decisions

- The failure marker is intentionally generic. The detailed failure reason stays in
  `agenthub doctor agent-trace` provider diagnostics, where it is already redacted and grouped with
  command-channel state.
- This change does not treat all no-response symptoms as submission failures. Provider turns,
  pending permissions, pending tool calls, SSE staleness, and frontend cache issues still require
  `agent-trace` classification first.

## Validation

```bash
cargo test -p agenthub acp_prompt_submission_failure_event_is_renderable_and_redacted -- --nocapture
cd web && npm exec vitest -- run src/pages/team_page.agent_loop.test.tsx
```

## Follow-Ups

- Continue the focused ACP long-session regression matrix for stale-session send recovery,
  permission history jump/copy, runtime metrics, and long output histories.
- When investigating a real no-response case, start with `agenthub doctor agent-trace` to classify
  whether the stall is provider, permission/tool, persistence, SSE, or frontend downstream state.
