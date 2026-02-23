---
title: Linkerdog Standalone Packages Split (Core / CLI / ACP)
date: 2026-02-23
status: implemented
---

## Summary

Restructure Linkerdog into three independent workspace packages:

- `linkerdog-core`: runtime/session engine library
- `linkerdog-cli`: user-facing binary (`linkerdog`)
- `linkerdog-acp`: ACP binary (`linkerdog-acp`)

The split keeps ACP compatibility while making runtime logic reusable and
separable from binary entrypoints.

## Background

The previous layout used one package (`agenthub-linkerdog-acp`) to hold both
runtime internals and binary entrypoint logic. That made deployment boundaries
blurry and made future expansion of non-ACP CLI behavior harder.

## Key Decisions

- Extract runtime/session implementation into `crates/linkerdog-core`.
- Keep ACP startup as a dedicated package `linkerdog-acp`:
  - parse ACP overrides (`-c key=value`)
  - run ACP runtime through `linkerdog-core`
- Add `linkerdog-cli` package for binary tool `linkerdog`:
  - `linkerdog` and `linkerdog acp` both route to ACP runtime for compatibility
- Keep old AgentHub ACP detection compatible and add first-class support for
  direct `linkerdog-acp` command matching.
- Update web preset command for Linkerdog to `linkerdog-acp` (no implicit
  subcommand required).

## Scope

- `Cargo.toml`
- `crates/linkerdog-core/Cargo.toml`
- `crates/linkerdog-core/BUILD.bazel`
- `crates/linkerdog-core/src/lib.rs`
- `crates/linkerdog-core/src/runtime.rs`
- `crates/linkerdog-core/src/agent.rs`
- `linkerdog-acp/Cargo.toml`
- `linkerdog-acp/BUILD.bazel`
- `linkerdog-acp/src/lib.rs`
- `linkerdog-acp/src/main.rs`
- `linkerdog-acp/README.md`
- `linkerdog-cli/Cargo.toml`
- `linkerdog-cli/BUILD.bazel`
- `linkerdog-cli/src/lib.rs`
- `linkerdog-cli/src/main.rs`
- `linkerdog-cli/README.md`
- `src/agent/manager/codec.rs`
- `src/agent/manager/tests.rs`
- `web/src/agent_presets.ts`
- `web/src/agent_presets.test.ts`
- `docs/todo.md`

## Validation

- [x] `cargo test -p linkerdog-core`
- [x] `cargo test -p linkerdog-acp`
- [x] `cargo test -p linkerdog-cli`
- [x] `cargo test -p agenthub acp_provider_for_agent_requires_expected_args`
- [x] `npm --prefix web test -- agent_presets.test.ts`
