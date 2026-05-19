---
name: agenthub-agent-debug
description: "Backend-first diagnosis for stuck AgentHub agents."
---

# AgentHub Agent Debug

Use this skill when an AgentHub agent or Team member stops replying, gets stuck in ACP/tool-call state, or loses output unexpectedly.

## Core Rule

Do not start from the browser unless backend evidence already shows persistence and SSE are healthy. Classify the stall layer first, then inspect the next boundary.

## Safety

- Treat diagnostics as read-only unless the user explicitly asks for a repair.
- Do not print prompt text, message bodies, tool raw input/output, environment variables, or tokens.
- Prefer IDs, timestamps, statuses, event types, cursor positions, and counts.
- Use debug-build-only diagnostic surfaces such as `agenthub doctor agent-trace`; do not expose new release-mode HTTP diagnostics without explicit design approval.

## Workflow

1. Identify the target.
   - Standalone agent: use `--agent-id <agent_id>`.
   - Team member: use `--team-id <team_id> --member-id <member_id>`.
   - If a session is known, include `--session-id <session_id>`.

2. Run the backend trace.

```bash
agenthub doctor agent-trace --agent-id <agent_id> --json
agenthub doctor agent-trace --team-id <team_id> --member-id <member_id> --json
agenthub doctor agent-trace --server-url http://127.0.0.1:8080 --token <root_session_token> --agent-id <agent_id> --json
```

Use `--server-url` when the debug-build AgentHub server is running and live runtime state is needed.
Without it, the command reads the SQLite snapshot only.

3. Classify the layer from the trace verdict.
   - `runtime_not_running`: inspect agent/session lifecycle before ACP.
   - `waiting_permission`: inspect permission request rows and timeout/deny handling.
   - `mailbox_pending`: inspect Team actor receive/ack flow and run ownership.
   - `no_persisted_events`: inspect provider adapter, ACP server, and event sink.
   - `event_stream_present`: inspect SSE broadcaster and frontend cache/rendering next.
   - `target_not_found` or `team_member_not_found`: fix target resolution first.

4. Verify persistence boundaries before UI work.
   - Confirm `agent_sessions.status`.
   - Confirm latest per-agent `agent_events` row for the session.
   - Confirm pending `acp_permission_requests` count.
   - Confirm pending `team_actor_messages` for Team members.

5. Only then use browser or Chrome DevTools MCP.
   - Use it to compare persisted event cursors with rendered ACP state.
   - If persisted events advance but UI is stale, inspect SSE and frontend cache handoff.
   - If persisted events do not advance, keep the investigation in backend/runtime code.

## Implementation Guidance

- Add focused regression tests for any fix.
- Keep diagnostic outputs redacted by construction; never collect bodies and then redact later if IDs/statuses are sufficient.
- If adding runtime instrumentation, prefer small domain structs and stable JSON output so the CLI can be scripted.
- For release builds, diagnostics must be compiled out or hard-disabled.

## Useful Commands

```bash
cargo test -p agenthub diagnostics::agent_trace
cargo test -p agenthub doctor_cli
cargo fmt --all --check
```
