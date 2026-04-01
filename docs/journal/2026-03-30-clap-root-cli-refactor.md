# Clap Root CLI Refactor

## Summary

- introduced a clap-based root parser for `agenthub`
- migrated `doctor` parsing/help onto clap
- routed `actor` through the new root parser while intentionally preserving the existing actor execution/parser layer

## Scope

This change is a staged CLI refactor, not a full actor subcommand rewrite.

### Included

- `agenthub --help` / `--version` now come from clap
- `agenthub doctor` now uses clap for parsing and help rendering
- `agenthub actor ...` now enters through the same clap root parser and then delegates to the existing actor parser/execution layer
- unknown top-level subcommands now fail fast instead of silently booting the HTTP server

### Deferred

- full clap migration of every `agenthub actor ...` subcommand and flag
- removal of the legacy hand-written actor help/parser layer

## Rationale

The previous root command flow used two independent `std::env::args()` probes (`doctor` and `actor`) before falling through to server startup. That had three maintainability problems:

1. top-level help/version behavior was effectively undefined
2. root command dispatch and subcommand help formatting were split across multiple ad-hoc entry points
3. unknown top-level argv could accidentally boot the server instead of failing as CLI input

This staged refactor narrows the problem first:

- one root parser owns the command boundary
- `doctor` becomes a complete clap command
- `actor` keeps its existing business parser until a dedicated follow-up change migrates it safely

## Validation

- focused unit coverage should exercise:
  - root parser `serve` / `doctor` / `actor` / legacy `actor-mcp`
  - clap help handling
  - doctor clap help / unknown-flag behavior

### Verified Evidence

- Focused validation for this change was captured in PR `#260` together with the parser/help tests
  and `cargo clippy`.
- `pull_request` CI for PR `#260`:
  - Bazel: `23750827017`
  - Rust: `23750827015`
  - Clippy: `23750827067`
  - Web: `23750827018`
  - Web E2E: `23750827012`
  - User Docs: `23750827019`
  - Distributed P2P Pipeline: `23750826999`
- default-branch `push` CI after merge commit `71420471`:
  - Bazel: `23751391475`
  - Rust: `23751391481`
  - Clippy: `23751391494`
  - Web: `23751391464`
  - Web E2E: `23751391438`
  - User Docs: `23751391447`
  - Distributed P2P Pipeline: `23751391513`
