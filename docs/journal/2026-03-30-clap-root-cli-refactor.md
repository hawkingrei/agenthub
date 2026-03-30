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
- CI evidence should be recorded here after merge:
  - push run id:
  - pr run id:
