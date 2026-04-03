## Summary

- add `codex_acp.multi_agent_enabled` as an explicit AgentHub config knob with a default of `true`
- pass the resolved policy into Codex ACP child sessions through `AGENTHUB_CODEX_ACP_MULTI_AGENT_ENABLED`
- stop relying on adapter-owned default multi-agent enablement inside `agenthub-codex-acp`

## Why

AgentHub should own whether Codex ACP sessions force-enable `Feature::Collab`. Keeping that behavior inside the adapter made the product contract implicit and left no clean way to disable the default in selected deployments.

This change keeps existing AgentHub behavior by default while making ownership explicit in the AgentHub config surface.

## Changes

- extend `CodexAcpConfig` with `multi_agent_enabled`
- thread the resolved boolean through `AppState` and `AgentManager` into Codex ACP process env assembly
- make `agenthub-codex-acp` apply `Feature::Collab` only when the AgentHub env override resolves to `true`
- document the new config knob and replace the old adapter-owned TODO/journal wording

## Validation

- `cargo test -p agenthub-config codex_acp_ -- --nocapture`
- `cargo test -p agenthub-codex-acp multi_agent -- --nocapture`
- `cargo test -p agenthub default_env_for_codex_provider -- --nocapture`
- `cargo clippy --locked -p agenthub-codex-acp --all-targets -- -D warnings`
- `cargo clippy --locked -p agenthub --all-targets -- -D warnings`
- `cargo fmt --all`
- `npm --prefix userdocs run build`
- `git diff --check`
