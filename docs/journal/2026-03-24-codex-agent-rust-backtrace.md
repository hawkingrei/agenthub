# Summary

Enable `RUST_BACKTRACE=1` by default for Codex ACP agent processes.

# Why

When a Codex-backed agent panics, the default child-process environment did not
guarantee a Rust backtrace. That made terminal debugging much harder, especially
for resumed ACP sessions and runtime panics that only appear in the managed
agent shell.

# What Changed

- Added provider-specific runtime env injection for ACP providers.
- Enabled `RUST_BACKTRACE=1` only for the Codex ACP provider.
- Kept Gemini and Kimi unchanged.
- Added focused tests for:
  - the Codex-only env selection rule
  - child-process env propagation inside the local executor

# Validation

- `cargo test -p agenthub default_env_for_codex_provider_enables_rust_backtrace -- --nocapture`
- `cargo test -p agenthub local_executor_applies_extra_env_pairs -- --nocapture`
- `cargo fmt --all --check`
