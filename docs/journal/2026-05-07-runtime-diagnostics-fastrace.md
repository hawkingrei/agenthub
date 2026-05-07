# Runtime Diagnostics Fastrace Bridge

## Summary

Added an opt-in fastrace bridge for AgentHub's existing `tracing` spans and corrected the node
detail runtime labels so attached-agent evidence is not presented as host binary detection.

## Background

The node detail panel previously rendered unavailable labels such as `Gemini CLI (not detected)`.
That wording was misleading because the UI only derived runtime evidence from attached agent
commands; it did not probe the host for an installed `gemini` binary.

Agent unresponsiveness investigations also need a wall-clock timeline view in addition to existing
logs and Pyroscope CPU profiling.

## Scope

- `agenthub-logging` now supports `AGENTHUB_FASTRACE=console` as an opt-in fastrace console
  reporter wired through `fastrace_tracing::FastraceCompatLayer`.
- The logging guard flushes fastrace on shutdown when the bridge is enabled.
- Node detail labels now say `no attached agent observed` instead of implying a failed host binary
  detection.
- `docs/features/runtime-diagnostics.md` captures the stable diagnostics contract.

## Key Decisions

- fastrace is disabled by default and only enabled through `AGENTHUB_FASTRACE`.
- The first reporter mode is console-only; production trace ingestion is out of scope.
- Runtime capability inventory for remote nodes remains a separate backend contract because node
  detail currently has no node-reported binary inventory.

## Validation

```bash
npm --prefix web run test -- src/components/agent_node_detail_shared.test.ts src/components/agent_nodes_workbench.test.tsx src/components/agent_node_section.test.tsx
npm --prefix web run lint
npm --prefix web exec tsc -- --noEmit --project web/tsconfig.json
npm --prefix web run build
cargo test -p agenthub-logging trace_export_options_parse_fastrace_console_env -- --nocapture
cargo test -p agenthub-logging init_tracing_supports_stdout_and_file_targets -- --nocapture
cargo test -p agenthub-logging
cargo check -p agenthub-logging
cargo fmt --all --check
```

## Follow-Ups

- Add node-reported runtime capability inventory so remote node binary availability can be shown
  without deriving it from attached agents.
- Add targeted spans near any runtime loop that still lacks enough wall-clock trace detail during a
  concrete unresponsive-agent investigation.
