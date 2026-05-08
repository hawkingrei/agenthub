# Agent Trace Diagnostics

## Summary

Added the first debug-build-only `agenthub doctor agent-trace` diagnostic slice for stuck AgentHub
agents and Team member ACP no-output investigations.

## Background

Agent Team ACP turns can appear stuck after a user sends input to a member agent. Before using the
browser as the primary evidence source, operators need a backend-first way to classify whether the
stall is in runtime ownership, persisted ACP events, permission review, mailbox delivery, or a later
SSE/frontend boundary.

## Scope

- Added a project skill, `agenthub-agent-debug`, that routes stuck-agent investigations through a
  backend trace first and only moves to Chrome DevTools MCP after persistence and SSE are proven
  healthy.
- Added the debug-only `agenthub doctor agent-trace` CLI surface for standalone agents and
  Team-scoped members.
- Added read-only SQLite snapshot diagnostics for agent rows, active sessions, per-agent ACP event
  cursors, pending permission requests, pending Team mailbox messages, and likely stall verdicts.
- Added human-readable and `--json` output.
- Hard-disabled the diagnostic command in release builds.
- Kept diagnostic output redacted by construction: IDs, timestamps, statuses, event types, and
  counts are allowed; prompt bodies, message bodies, tool arguments, tool output bodies, environment
  values, and provider tokens are not emitted.
- Added a debug-build-only, root-authenticated live backend overlay at
  `/api/diagnostics/agent_trace`, reachable from `agenthub doctor agent-trace --server-url <url>
  --token <root-session-token>`, for live runtime handle, ACP command-channel, prompt queue, and
  output broadcast subscriber state.
- Extended the live overlay with provider-neutral adapter and SSE delivery accounting: active
  submission ids, last submission id, last provider event class/timestamp, pending tool-call
  ids/statuses, last command error metadata, active SSE stream/forwarder counts, last forwarded and
  emitted event ids/timestamps, and last SSE delivery error metadata.

## Key Decisions

- The first slice intentionally uses read-only SQLite snapshots so it is safe for local diagnosis
  and AgentHub-managed agents.
- Provider-adapter live progress and SSE broadcaster freshness are not fabricated from the database
  snapshot. The live overlay only reports state that the backend can read directly from live runtime
  handles and SSE forwarders. Provider-native turn ids remain adapter-specific and should be added
  only when the adapter can expose them without serializing prompt or tool payload bodies.
- The workflow remains backend-first: browser inspection is downstream evidence, not the first
  step, unless the backend trace already proves persistence and emission are healthy.

## Validation

```bash
python3 /Users/weizhenwang/.codex/skills/.system/skill-creator/scripts/quick_validate.py .agents/skills/agenthub-agent-debug
cargo fmt --all --check
cargo test -p agenthub diagnostics::agent_trace
cargo test -p agenthub doctor_cli
cargo test -p agenthub-acp acp_handle_send_times_out_when_channel_is_backpressured
cargo test -p agenthub-acp acp_runtime_diagnostics_tracks_redacted_live_state
cargo test -p agenthub-acp acp_handle_send
cargo test -p agenthub sse::tests::output_stream_emits_events_from_forwarders
cargo check -p agenthub
cargo clippy -p agenthub --all-targets -- -D warnings
gh pr checks 559 | cat
```

PR #559 also resolved the review feedback for read-only SQLite busy timeout, blank `agent_id`
normalization, unnecessary Team member clone, and UUID-backed temporary test directories.

## Follow-Ups

- Add provider-native turn ids when a provider adapter exposes them as safe metadata. The current
  live overlay uses AgentHub submission ids as the provider-neutral correlation point.
- After the live backend path lands, run Chrome DevTools MCP only for cases where persisted ACP
  events and SSE emission are both healthy but the Team ACP UI still renders stale state.
