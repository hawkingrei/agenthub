# Generic Codex ACP Entrypoint

## Summary

AgentHub now supports `agenthub-acp codex` as the canonical Codex ACP adapter command while keeping
`agenthub-codex-acp` packaged and recognized for existing configurations. The generic adapter
entrypoint dispatches to the same AgentHub Codex ACP implementation, so the provider boundary stays
ACP JSON-RPC over the child process stream and does not fork Codex runtime behavior.

## Background

`agenthub-acp claude` introduced the provider-selected adapter binary. Codex still used the older
dedicated binary as the default preset, leaving the generic adapter incomplete and forcing operators
to remember provider-specific binary names. This rollout folds Codex into the generic command
without removing the compatibility binary.

## Scope

- Add an `agenthub-acp codex` subcommand that forwards Codex `-c/--config key=value` overrides.
- Detect `agenthub-acp codex` as the Codex provider in the backend and web UI.
- Keep `agenthub-codex-acp` and `codex-acp` as recognized compatibility commands.
- Change the default Codex web preset to `agenthub-acp codex`.
- Keep release packaging for both `agenthub-acp` and `agenthub-codex-acp`.
- Carry Codex release-only vendored OpenSSL through the `agenthub-acp-adapter` cross build.

## Key Decisions

- The generic command reuses `agenthub_codex_acp::run_main` instead of moving Codex protocol logic
  into the generic adapter crate.
- The compatibility `agenthub-codex-acp` binary remains the path that owns upstream Codex `arg0`
  dispatch behavior. `agenthub-acp codex` is the AgentHub-managed adapter entrypoint.
- AgentHub must not rewrite `agenthub-acp codex` to `codex_acp.binary`; that override remains for
  legacy or custom Codex ACP commands.
- `agenthub-codex-acp` stays in release artifacts until a later reviewed deprecation window.

## Validation

Focused checks:

```bash
cargo test -p agenthub-acp-adapter
cargo check -p agenthub-acp-adapter
cargo test acp_provider_for_agent_requires_expected_args -- --nocapture
npm --prefix web run test -- src/agent_presets.test.ts src/components/agent_node_detail_shared.test.ts
npm --prefix web run build
npm --prefix userdocs run build
cargo fmt --check
git diff --check
```

## Follow-Ups

- Run real long-session smoke coverage for Codex and non-Codex ACP providers under the existing
  long-session browser verification TODO.
