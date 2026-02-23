---
title: Linkerdog Standalone ACP Package
date: 2026-02-23
status: implemented
---

## Summary

Introduce an independent Rust package `agenthub-linkerdog-acp` that provides
standalone executable `linkerdog` and supports both invocation forms:

- `linkerdog`
- `linkerdog acp`

This enables Linkerdog ACP to run independently from the AgentHub server
binary with its own native ACP runtime.

## Background

AgentHub already supports provider detection for `linkerdog acp`, but
`linkerdog` originally delegated directly into `agenthub-codex-acp` runtime.
We need a self-owned runtime boundary so Linkerdog can evolve independently in:

- multi-provider/model policy;
- context persistence;
- tool-call orchestration and permission policy.

## Key Decisions

- Add new workspace member: `agenthub-linkerdog-acp`.
- Expose binary name as `linkerdog` (not `agenthub-linkerdog-acp`) for
  provider preset and CLI ergonomics.
- Replace codex runtime delegation with native ACP runtime:
  - own `LinkerdogAgent` implementation;
  - own session state/model/mode/config management;
  - own prompt/tool-call handling loop.
- Normalize CLI args by stripping a leading `acp` subcommand token so both
  `linkerdog` and `linkerdog acp` start the same ACP runtime.
- Implement provider/model/mode selectors as ACP session capabilities:
  - modes: `ask`/`code`/`review`;
  - providers: `openai`/`anthropic`/`google`/`deepseek`;
  - provider-scoped model list and runtime validation.
- Implement append-only context persistence in workspace:
  - `<cwd>/.cache/context/run/<session_id>/state.json`;
  - `<cwd>/.cache/context/run/<session_id>/history.jsonl`.
- Implement base tool-call flow:
  - `/tool exec <command>` triggers ACP tool call lifecycle;
  - request permission (`allow_once`/`reject_once`);
  - execute command locally and emit tool-call update content.
- Add Bazel targets for library, binary, and unit tests.

## Scope

- `Cargo.toml`
- `agenthub-linkerdog-acp/Cargo.toml`
- `agenthub-linkerdog-acp/src/main.rs`
- `agenthub-linkerdog-acp/src/lib.rs`
- `agenthub-linkerdog-acp/src/runtime.rs`
- `agenthub-linkerdog-acp/src/agent.rs`
- `agenthub-linkerdog-acp/BUILD.bazel`
- `agenthub-linkerdog-acp/README.md`
- `docs/todo.md`

## Validation

- [x] `cargo test -p agenthub-linkerdog-acp`
- [x] `cargo test -p agenthub acp_provider_for_agent_requires_expected_args`
- [x] `cargo run -p agenthub-linkerdog-acp -- --help`
- [x] `cargo run -p agenthub-linkerdog-acp -- acp --help`
