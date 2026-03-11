# Make Run Server Entrypoint Clarification

## Summary

Clarified local development entrypoints so `make run` is explicitly the AgentHub server entrypoint while keeping the project single-binary.

## Why

- AgentHub now exposes multiple subcommands from the same `agenthub` binary (`actor`, `actor-mcp`, and future Team/runtime helpers).
- `make run` should remain the unambiguous path for starting the web/API server.
- We do not want to introduce extra binaries or separate deployment artifacts.

## What Changed

- `Makefile`
  - `run` now aliases `run-server`
  - `run-server` explicitly executes `cargo run --`
- `README.md`
  - Quick start now uses `make run`
  - Added explicit note that actor CLI and actor MCP are subcommands of the same binary

## Validation

Recommended checks:

```bash
make -n run
cargo run -- actor --help
cargo run -- actor-mcp --help
```

## Scope

- `Makefile`
- `README.md`
