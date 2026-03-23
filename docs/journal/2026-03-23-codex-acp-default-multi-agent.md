## Summary

Enable Codex subagent tools by default in `agenthub-codex-acp` so AgentHub ACP sessions do not
depend on each user's `~/.codex/config.toml` setting `features.multi_agent = true`.

## Why

AgentHub relies on Codex ACP as an adapter layer. Requiring every machine to opt into
`multi_agent` in a local Codex config makes subagent availability drift across environments and
creates a hidden per-user prerequisite for Team sessions.

The adapter already owns other AgentHub-specific normalization steps, so defaulting
`Feature::Collab` here is the smallest place to make subagent support deterministic.

## What Changed

- Added adapter-side feature normalization in `agenthub-codex-acp/src/lib.rs`:
  - if `Feature::Collab` is currently off, try to enable it before constructing the ACP agent
  - leave upstream runtime-local guards intact (for example review flows and depth-based disablement)
  - if feature constraints reject the enablement, keep startup alive and log the failure
- Added unit tests covering:
  - default enablement when `Collab` is off
  - no-op behavior when `Collab` is already on
  - surfaced failure path for constrained feature states
- Updated `docs/features/acp-runtime.md` to record the adapter default
- Added a follow-up TODO to decide whether this should later move into an explicit AgentHub config
  knob instead of remaining adapter-owned behavior

## Validation

- `cargo test -p agenthub-codex-acp --lib`
- `cargo clippy --locked -p agenthub-codex-acp --all-targets -- -D warnings`
- `cargo fmt --all --check`
- `git -c core.fsmonitor=false diff --check`
