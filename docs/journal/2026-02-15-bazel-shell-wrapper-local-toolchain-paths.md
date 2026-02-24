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

- `BUILD.bazel`
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
4. Set Bazel target timeout for `//:rust_test` to `long` in `BUILD.bazel`:
   - CI showed Rust tests completing around 298s; default 300s timeout caused
     flaky timeout failures even when all test cases were passing.

## Validation

```bash
USE_BAZEL_VERSION=9.0.0 bazel --output_user_root=/tmp/agenthub-bazel-root-check6 build --repository_cache=/tmp/agenthub-bazel-repo-cache-check6 --disk_cache=/tmp/agenthub-bazel-disk-cache-check6 //...
USE_BAZEL_VERSION=9.0.0 bazel --output_user_root=/tmp/agenthub-bazel-root-ci-timeout-fix test --repository_cache=/tmp/agenthub-bazel-repo-cache-ci-timeout-fix --disk_cache=/tmp/agenthub-bazel-disk-cache-ci-timeout-fix --test_output=errors //:ci_tests
```

Observed results:

- `bazel build //...`: `INFO: Build completed successfully` (`Found 6 targets`)
- `bazel test //:ci_tests`: `2 tests pass` with `//:rust_test` completing in
  ~298s (no timeout)
