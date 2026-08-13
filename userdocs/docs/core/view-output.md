---
sidebar_position: 4
---

# View Execution Output

AgentHub renders ACP output as a structured timeline while preserving raw
runtime events for recovery and diagnosis.

## Main Views

- **Thread**: user and agent messages.
- **Plan**: structured plan updates when the provider emits them.
- **Debug**: raw ACP and system detail. This tab is hidden in production until
  developer mode is enabled from **Admin**.

Provider-specific tool calls, permissions, modes, models, and configuration
controls appear only when the active runtime advertises them.

## Connection Badge

The workspace header distinguishes browser network state from the live stream:

- `Online · SSE connected`: live events are arriving normally.
- `Online · SSE connecting` or `reconnecting`: the UI is restoring the stream.
- `Online · SSE idle`: there is no running stream target.
- `Offline · SSE disconnected`: the browser reports no network connection.

A reconnecting badge does not mean history was lost. The UI falls back to the
persisted event API while the SSE connection recovers.

## History Replay

When you reopen an agent, AgentHub loads recent events and lets you request
older pages. The event API returns at most 20 records per request and uses a
`before_id` cursor for older history.

Large ACP tool payloads may be compacted in the live stream. The UI can fetch
the complete persisted event by its event ID when detail is needed.

## Review Tips

- Read the newest actionable Thread or Plan block first.
- Open Debug only when structured rendering is insufficient.
- Confirm the agent status before treating a quiet stream as a connection
  failure.
- Refreshing the page is safe; it does not stop the backend process.
