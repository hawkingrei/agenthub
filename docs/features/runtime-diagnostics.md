# Runtime Diagnostics

## Problem

Long-lived agents can stop responding while their subprocess, ACP session, mailbox relay, or
runtime loop still appears alive. Operators need low-friction diagnostics that distinguish timing
stalls from CPU hotspots and avoid relying on UI status alone.

## Scope

- Process-wide structured logs through the existing `tracing` subscriber.
- Optional timeline trace export from `tracing` spans through fastrace.
- Process-wide CPU profiling through the existing Pyroscope integration.
- Node detail runtime labels that describe observed attached-agent evidence without claiming host
  binary installation status.
- Codex ACP adapter diagnostics that identify live turn/tool-call completeness gaps before they
  surface as agent unresponsiveness or Codex core panics.
- Agent-output stall diagnostics that correlate frontend-visible output state, backend session
  state, persisted event cursors, SSE freshness, fallback refreshes, and provider-adapter progress.
- A read-only diagnostic CLI surface for operators and AgentHub-managed agents to collect a compact
  trace bundle for one stuck agent or Team member without manually joining database, log, and ACP
  state by hand.

## Non-Goals

- fastrace is not enabled by default.
- fastrace is not a replacement for Pyroscope CPU profiling.
- Node detail runtime labels are not a remote host capability probe.
- Remote node binary inventory is out of scope until nodes explicitly report capabilities.
- Codex-native diagnostics do not replace the ACP runtime boundary for normal prompt, permission,
  event, or UI flows.
- The diagnostic CLI must not repair, restart, interrupt, or mutate stuck sessions by default.
- The diagnostic CLI is not a general log-ingestion system and should not persist full prompt,
  message, or tool-output bodies unless an explicit debug export mode is added later.

## Architecture

`agenthub-logging` owns the logging and trace subscriber setup. The default startup path keeps the
existing formatted log output. When `AGENTHUB_FASTRACE` is set to `console`, `true`, `1`, or
`stdout`, AgentHub also installs `fastrace_tracing::FastraceCompatLayer` and a fastrace console
reporter.

This bridge captures existing `tracing` spans, including agent runtime/session spans, as timeline
records. It is meant for short diagnostic sessions where stdout or the configured log target can be
collected for stall analysis.

Pyroscope remains the profiler path for CPU-heavy symptoms. Use fastrace when the question is
"where did the runtime spend wall-clock time" and Pyroscope when the question is "where did the CPU
go".

Codex ACP diagnostics are adapter-local state snapshots. `agenthub-codex-acp` may read upstream
Codex app-server/thread state to explain a Codex-backed stall, but the main AgentHub runtime should
still treat ACP as the provider-neutral control and event protocol. The diagnostic snapshot should
be small enough to attach to runtime logs, debug surfaces, or future support bundles without
replaying full conversation content.

Agent-output stall diagnostics sit above provider-specific adapter state. The canonical question is
not only "is the process alive", but where the visible-output pipeline stopped:

1. runtime process/session ownership;
2. provider adapter turn progress;
3. ACP event persistence;
4. SSE delivery to the browser;
5. frontend cache/render state;
6. Team mailbox or permission-review gating.

The first CLI surface should be read-only, for example `agenthub doctor agent-trace`. It should
accept either a standalone `agent_id` or a Team-scoped `team_id + member_id`, resolve the active
AgentHub session and provider continuity id, then print a compact timeline and machine-readable JSON
summary. The command should be useful both to a human operator and to an AgentHub-managed agent that
needs to inspect why its own UI output appears stuck.

## Contracts

- `AGENTHUB_FASTRACE` unset or unrecognized: fastrace remains disabled.
- `AGENTHUB_FASTRACE=console`: fastrace writes timeline records through its console reporter.
- `AGENTHUB_FASTRACE=true`, `1`, or `stdout`: aliases for console mode.
- Shutdown drops the logging guard and flushes fastrace before process exit.
- Node detail runtime tags with `no attached agent observed` only mean no current attached agent
  command proved that runtime; they do not mean the CLI is missing from the host.
- For Codex-backed ACP sessions, diagnostic snapshots should include:
  - provider session id and AgentHub session id
  - active Codex thread id, turn id, and AgentHub submission id when available
  - queued prompt/session-mutation counts
  - pending app-server request ids grouped by request kind
  - tool-call completeness state keyed by call id, including `started`, `output_seen`, and terminal
    status
  - the last Codex event class observed by the adapter
- A missing `CustomToolCallOutput` must be reported as a recoverable diagnostic finding before
  compaction, resume normalization, or turn finalization can panic.
- Agent-output stall diagnostics should report these provider-neutral fields:
  - resolved `agent_id`, optional `team_id`, optional `member_id`, AgentHub `session_id`, and
    provider continuity/session id when known
  - process/runtime status, runtime owner node, last process heartbeat or exit observation, and
    stale-running reconciliation status
  - latest persisted `agent_events.id`, latest event timestamp, latest ACP event type, and latest
    renderable message/tool-call summary
  - latest browser-delivery evidence when available: active SSE target, last SSE event timestamp,
    fallback polling state, and latest fetched event cursor
  - pending permission-review requests, pending mailbox messages that can block the next turn, and
    pending tool calls grouped by call id/status
  - adapter-local progress fields for provider-backed sessions, including active turn/submission id,
    queued prompt count, and last provider event class/timestamp
- The CLI summary should classify the most likely stall layer as one of:
  - `runtime_not_running`
  - `provider_turn_waiting`
  - `permission_or_tool_waiting`
  - `events_not_persisted`
  - `sse_delivery_stale`
  - `frontend_render_stale`
  - `unknown`
- The CLI must support `--json` for agent-consumable output and a human-readable default table or
  timeline.
- Diagnostic output must redact prompt bodies, message bodies, tool arguments, environment values,
  and provider tokens by default. IDs, timestamps, statuses, event classes, counts, and short
  synthetic summaries are allowed.

## Validation Matrix

- `cargo test -p agenthub-logging trace_export_options_parse_fastrace_console_env -- --nocapture`
- `cargo test -p agenthub-logging init_tracing_supports_stdout_and_file_targets -- --nocapture`
- `npm --prefix web run test -- src/components/agent_node_detail_shared.test.ts src/components/agent_nodes_workbench.test.tsx src/components/agent_node_section.test.tsx`
- `npm --prefix web run lint`
- `npm --prefix web exec tsc -- --noEmit --project web/tsconfig.json`
- `cargo test -p agenthub-codex-acp` focused on live-turn diagnostic accounting and dirty custom
  tool-call history repair.
- Focused CLI tests should cover agent-output stall summaries for:
  - a live session with no newly persisted events after a recent input;
  - a pending permission/tool-call gate;
  - a stale SSE cursor while persisted events advanced;
  - a stale `running` row with no live runtime handle;
  - redaction of prompt/tool payload bodies in both text and `--json` output.

## Operational Notes

For an unresponsive agent investigation, start with:

```bash
AGENTHUB_FASTRACE=console RUST_LOG=agenthub=debug,agenthub_acp=debug,agenthub_team_actor=debug agenthub
```

Keep the capture window short. The console reporter is intentionally simple and can be noisy on a
busy process.

For Codex-specific stalls, collect the ACP debug raw events plus the Codex ACP diagnostic snapshot.
The first triage question should be whether the adapter is waiting on prompt completion, a
permission/tool response, an app-server request, or a tool call whose output was never observed.

For browser-visible output stalls, collect the agent-trace summary first, then use Chrome DevTools
MCP only for the browser layer:

1. run the read-only CLI for the selected agent or Team member;
2. compare the latest persisted event id/timestamp with the browser's last fetched cursor;
3. inspect active EventSource targets and repeated `/api/agents/:id/events?...` requests;
4. if persisted events are advancing but the UI is stale, focus on SSE/cache/render state;
5. if persisted events are not advancing, focus on provider adapter, permission/tool gating, or
   runtime ownership.

## Open Risks

- Console output is suitable for local diagnosis, not production trace ingestion.
- fastrace trace quality depends on existing `tracing` span coverage. Missing spans in critical
  runtime loops should be added near the stalled path being investigated.
- Codex diagnostic state can expose adapter/accounting gaps, but it must avoid storing large prompt
  or tool-output payloads by default.
- Remote node capability reporting still needs a separate node-reported contract.
- Browser-visible output stalls can involve state that only exists in the current browser tab.
  Backend CLI diagnostics should make that gap explicit instead of claiming full frontend truth.
- Some provider adapters may not expose enough native turn state yet. The provider-neutral CLI should
  report missing adapter diagnostics as `unknown` rather than fabricating progress.

## Source Journals

- [2026-03-26 AgentHub Pyroscope Bootstrap](../journal/2026-03-26-agenthub-pyroscope-bootstrap.md)
- [2026-04-24 Codex Custom Tool Output Hotfix](../journal/2026-04-24-codex-custom-tool-output-hotfix.md)
- [2026-05-04 Node Detail Runtime Labels](../journal/2026-05-04-node-detail-runtime-labels.md)
- [2026-05-07 Runtime Diagnostics Fastrace Bridge](../journal/2026-05-07-runtime-diagnostics-fastrace.md)
