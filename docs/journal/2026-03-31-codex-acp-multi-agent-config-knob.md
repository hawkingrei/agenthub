## Summary

Move Codex ACP multi-agent enablement from adapter-owned default behavior to an explicit
AgentHub config knob: `codex_acp.multi_agent_enabled`.

## Why

The earlier adapter-side default made subagent availability deterministic, but it still left
ownership of the policy in `agenthub-codex-acp`. AgentHub should own that decision because it is
part of the product-level runtime contract, not a standalone adapter opinion.

An explicit AgentHub knob also makes it possible to disable forced `Feature::Collab` in specific
deployments while preserving the existing default for normal AgentHub-managed sessions.

## What Changed

- Added `codex_acp.multi_agent_enabled` to the AgentHub config schema, with a default of `true`.
- Threaded the resolved boolean through `AppState` -> `AgentManager` -> local execution request
  environment assembly.
- AgentHub now launches Codex ACP sessions with an explicit child-process env override:
  `AGENTHUB_CODEX_ACP_MULTI_AGENT_ENABLED=1|0`.
- `agenthub-codex-acp` now reads that override and only forces `Feature::Collab` when the env
  explicitly resolves to `true`.
- Missing or explicit `false` overrides leave the adapter feature set untouched.
- Invalid override values log a warning and are ignored.

## Validation

- `cargo test -p agenthub-config codex_acp_ -- --nocapture`
- `cargo test -p agenthub-codex-acp multi_agent -- --nocapture`
- `cargo test -p agenthub default_env_for_codex_provider -- --nocapture`
- `cargo clippy --locked -p agenthub --all-targets -- -D warnings`
- `cargo clippy --locked -p agenthub-codex-acp --all-targets -- -D warnings`
- `cargo fmt --all --check`
- `git diff --check`
