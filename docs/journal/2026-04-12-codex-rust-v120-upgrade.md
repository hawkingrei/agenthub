# Codex Rust v0.120.0 Upgrade

## Goal

Upgrade `agenthub-codex-acp` from the official `openai/codex` `rust-v0.118.0` baseline to
`rust-v0.120.0`.

## Source

- official release tag: `rust-v0.120.0`
- annotated tag object: `84b1753e16766434a86ec29ab7a23984fd0f61fe`
- pinned release commit: `65319eb1400cbd2890c43d572263dabd25f18ba9`

## Change

- updated all direct `agenthub-codex-acp` git dependencies from the previous `openai/codex`
  release commit `b630ce9a4e754d35a1f33e4366ba638d18626142` to
  `65319eb1400cbd2890c43d572263dabd25f18ba9`
- added direct `codex-config` and `codex-models-manager` dependencies because v0.120.0 no longer
  exposes those types through the old `codex-core` paths used by `agenthub-codex-acp`
- updated `agenthub-codex-acp` compatibility code for the v0.120.0 API surface:
  - auth and models-manager imports now use `codex-login` and `codex-models-manager`
  - `ThreadManager::new(...)` and `InProcessClientStartArgs` now receive the new optional
    analytics/environment arguments
  - `Op::UserInput`, `TurnStartParams`, `TurnSteerParams`, `TurnStartedEvent`,
    `TurnCompleteEvent`, `TurnAbortedEvent`, and app-server `Turn` test fixtures now populate the
    new metadata/timing fields
  - prompt/event translation now tolerates the new realtime notification and history variants
- kept the repository pinned by commit instead of symbolic tag so dependency provenance stays
  stable

## Validation

Executed validation for this slice:

```bash
cargo fmt --all
cargo check -p agenthub-codex-acp
cargo test -p agenthub-codex-acp
```

Result:

- `cargo check -p agenthub-codex-acp` passed
- `cargo test -p agenthub-codex-acp` passed (`85 passed; 0 failed`)
