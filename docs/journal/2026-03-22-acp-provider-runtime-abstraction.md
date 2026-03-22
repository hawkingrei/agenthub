---
title: ACP Provider, Runtime Placement, and Proxy Layering
date: 2026-03-22
status: implemented
---

## Summary

Make the ACP runtime boundary more explicit before adding remote-node/P2P support:

- consolidate provider metadata into a single provider spec;
- make runtime placement explicit as `LocalProcess`;
- keep proxy handling provider-agnostic instead of Codex-specific;
- move local subprocess spawning behind `AgentExecutor` / `LocalExecutor`.

This change is intentionally behavior-preserving. ACP sessions still run as local
child processes, and Codex/Gemini/Kimi keep their existing command semantics.

## Motivation

The codebase already supports multiple ACP-capable CLIs, but the runtime shape
was still implicitly "local subprocess + provider-specific branching". That
would make future remote-node support harder because transport concerns could
start leaking into provider-specific code.

Before implementing P2P execution, AgentHub needs one stable split:

1. provider adapter semantics;
2. runtime placement semantics;
3. proxy / egress semantics;
4. local execution orchestration.

## Decision

- Introduce an `AcpProviderSpec` to hold provider id, prompt-delivery policy,
  and default-mode behavior in one place.
- Keep `codex`, `gemini`, and `kimi` differences inside provider resolution.
- Introduce explicit ACP runtime placement with a local-process baseline.
- Wrap proxy environment pairs in a provider-agnostic proxy policy abstraction
  so the egress policy can later be reused for remote runtimes.
- Introduce `AgentExecutor` with a `LocalExecutor` implementation so
  `AgentManager` no longer owns raw subprocess spawn wiring directly.
- Build an explicit `AgentStartPlan` before reservation so local reuse versus
  remote start is visible as a first-class decision boundary.

## Scope

- `src/agent/manager.rs`
- `src/agent/manager/acp_provider.rs`
- `src/agent/manager/executor.rs`
- `src/agent/manager/start_plan.rs`
- `src/agent/manager/tests.rs`
- `crates/agenthub-acp/src/lib.rs`
- `docs/features/acp-runtime.md`
- `docs/todo.md`

## Follow-ups

- Extend ACP runtime placement beyond `LocalProcess` with a remote-node/P2P
  connector.
- Refactor ACP session bootstrap away from `ChildStdin` / `ChildStdout` so the
  same session plumbing can support tunneled or remote transports.
- Move from launch-time env injection only to a reusable proxy policy that can
  also describe remote-node egress settings.

## Validation

- `cargo test -p agenthub acp_provider_for_agent_requires_expected_args`
- `cargo test -p agenthub runtime_location_defaults_to_local_process`
- `cargo test -p agenthub-acp acp_runtime_location_defaults_to_local_process`
- `cargo check --workspace --locked`
