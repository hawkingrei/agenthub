## Summary

- add a clap-based root parser for `agenthub`
- migrate `agenthub doctor` parsing/help to clap
- route `agenthub actor ...` through the new root parser while preserving the existing actor parser/execution layer

## Why

The previous root command flow used two independent `std::env::args()` probes and silently fell through to server startup for unknown top-level argv. That left root help/version behavior undefined and duplicated CLI dispatch logic.

This change narrows the problem first:

- one root parser owns top-level command dispatch
- `doctor` becomes a complete clap command
- `actor` keeps its current behavior, but now enters through the same root boundary

## Scope

Included in this PR:

- `agenthub --help` / `--version` now come from clap
- `agenthub doctor` now uses clap parsing and clap-rendered help
- `agenthub actor ...` now goes through the root clap parser before delegating to the existing actor parser
- unknown top-level subcommands now fail fast instead of booting the server

Deferred to follow-up:

- full clap migration for every `agenthub actor ...` subcommand and flag
- removal of the legacy hand-written actor help/parser layer

## Validation

- `cargo test -p agenthub cli::tests:: -- --nocapture`
- `cargo clippy -p agenthub --all-targets -- -D warnings`
- `git diff --check`
