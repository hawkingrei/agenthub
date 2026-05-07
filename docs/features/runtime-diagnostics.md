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

## Non-Goals

- fastrace is not enabled by default.
- fastrace is not a replacement for Pyroscope CPU profiling.
- Node detail runtime labels are not a remote host capability probe.
- Remote node binary inventory is out of scope until nodes explicitly report capabilities.

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

## Contracts

- `AGENTHUB_FASTRACE` unset or unrecognized: fastrace remains disabled.
- `AGENTHUB_FASTRACE=console`: fastrace writes timeline records through its console reporter.
- `AGENTHUB_FASTRACE=true`, `1`, or `stdout`: aliases for console mode.
- Shutdown drops the logging guard and flushes fastrace before process exit.
- Node detail runtime tags with `no attached agent observed` only mean no current attached agent
  command proved that runtime; they do not mean the CLI is missing from the host.

## Validation Matrix

- `cargo test -p agenthub-logging trace_export_options_parse_fastrace_console_env -- --nocapture`
- `cargo test -p agenthub-logging init_tracing_supports_stdout_and_file_targets -- --nocapture`
- `npm --prefix web run test -- src/components/agent_node_detail_shared.test.ts src/components/agent_nodes_workbench.test.tsx src/components/agent_node_section.test.tsx`
- `npm --prefix web run lint`
- `npm --prefix web exec tsc -- --noEmit --project web/tsconfig.json`

## Operational Notes

For an unresponsive agent investigation, start with:

```bash
AGENTHUB_FASTRACE=console RUST_LOG=agenthub=debug,agenthub_acp=debug,agenthub_team_actor=debug agenthub
```

Keep the capture window short. The console reporter is intentionally simple and can be noisy on a
busy process.

## Open Risks

- Console output is suitable for local diagnosis, not production trace ingestion.
- fastrace trace quality depends on existing `tracing` span coverage. Missing spans in critical
  runtime loops should be added near the stalled path being investigated.
- Remote node capability reporting still needs a separate node-reported contract.

## Source Journals

- [2026-03-26 AgentHub Pyroscope Bootstrap](../journal/2026-03-26-agenthub-pyroscope-bootstrap.md)
- [2026-05-04 Node Detail Runtime Labels](../journal/2026-05-04-node-detail-runtime-labels.md)
- [2026-05-07 Runtime Diagnostics Fastrace Bridge](../journal/2026-05-07-runtime-diagnostics-fastrace.md)
