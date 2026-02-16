# Rust Toolchain Components for Local Compile

## Summary

Expand rustup component declarations in workspace toolchain files so local
compilation no longer depends on manually installed default components.

## Background

The workspace pins Rust `1.93.1` but previously declared only a subset of
components (`clippy`, `rustfmt`, `rust-src`). On some local environments this
caused compile failures because `rustc`, `cargo`, or standard library
components were not present under the selected toolchain.

## Scope

- `rust-toolchain.toml`
- `agenthub-codex-acp/rust-toolchain.toml`
- `docs/todo.md`

## Key Decisions

1. Keep both workspace and ACP subproject toolchain files aligned.
2. Declare compile-critical components explicitly in toolchain files instead of
   relying on host defaults.
3. Preserve existing Rust version pin and only adjust component installation
   behavior.

## Validation

```bash
rustup show
cargo check --workspace --locked
```

Expected:

- Selected toolchain includes required compile components without extra manual
  steps.
- Workspace compiles on a clean local machine using repository toolchain files.

## Follow-ups

- If we later adopt a stricter toolchain bootstrap script, keep it sourced from
  `rust-toolchain.toml` to avoid drift between local and CI environments.
