# Bazel Shell Wrapper Local Toolchain Paths

## Summary

Harden Bazel shell wrapper scripts so local macOS toolchain paths are discovered
without extra command-line env flags, and ensure Cargo has a writable home in
restricted sandbox environments.

## Background

The Bazel CI integration currently uses shell-wrapper targets (`workspace_shell_*`)
that call `npm` and `cargo` directly. On local macOS setups, Bazel action PATH
may not include Homebrew or rustup bin paths by default, causing:

- `missing required command: npm`
- `missing required command: cargo`

In restricted action environments, Cargo cache writes under `~/.cargo` may also
fail with permission errors.

## Scope

- `bazel/ci/web_build.sh`
- `bazel/ci/web_test.sh`
- `bazel/ci/rust_build.sh`
- `bazel/ci/rust_test.sh`
- `docs/todo.md`

## Key Decisions

1. Extend npm path candidates with common local locations:
   - `/opt/homebrew/bin`
   - `/opt/homebrew/opt/node/bin`
   - `/usr/local/bin`
2. Extend cargo path detection with username-derived rustup paths:
   - `/Users/<user>/.cargo/bin`
   - `/home/<user>/.cargo/bin`
   - plus existing CI paths.
3. Add `CARGO_HOME` writable fallback:
   - if detected `CARGO_HOME` is not writable, auto-fallback to a temp dir in
     `${TMPDIR:-/tmp}` so Rust build/test can proceed inside restricted action
     sandboxes.

## Validation

```bash
USE_BAZEL_VERSION=9.0.0 bazel --output_user_root=/tmp/agenthub-bazel-root-check6 build --repository_cache=/tmp/agenthub-bazel-repo-cache-check6 --disk_cache=/tmp/agenthub-bazel-disk-cache-check6 //...
```

Observed result: `INFO: Build completed successfully` with `Found 6 targets`.
