# Bazel Actor CLI Path Canonicalization

## Summary

Canonicalize `default_actor_cli_path()` so actor runtime context and related API
tests use a stable executable path format under Bazel/macOS.

## Background

When tests run via Bazel on macOS, `std::env::current_exe()` may return a path
under `/var/...` while canonicalization resolves to `/private/var/...`. Actor
runtime code compared canonical and non-canonical values in different branches,
which caused path mismatch assertions in:

- `api::agents::tests::parse_start_actor_runtime_context_accepts_valid_payload`
- `api::agents::tests::start_route_with_actor_runtime_payload_injects_actor_envs`
- `api::teams::tests::start_agent_with_actor_context_injects_runtime_env_vars`

## Scope

- `src/actor_runtime.rs`
- `docs/todo.md`

## Key Decisions

1. Keep normalization policy centralized in `default_actor_cli_path()`.
2. Return canonical executable path by default so both:
   - explicit actor CLI path inputs, and
   - implicit default actor CLI path
   resolve to the same path identity.

## Validation

```bash
USE_BAZEL_VERSION=9.0.0 bazel --output_user_root=/tmp/agenthub-bazel-root-fresh test --test_output=errors --repository_cache=/tmp/agenthub-bazel-repo-cache-fresh --disk_cache=/tmp/agenthub-bazel-disk-cache-fresh --action_env=PATH="$PATH" --action_env=CARGO_HOME=/tmp/agenthub-cargo-home --action_env=RUSTUP_HOME=$HOME/.rustup //:ci_tests
USE_BAZEL_VERSION=9.0.0 bazel --output_user_root=/tmp/agenthub-bazel-root-fresh build --repository_cache=/tmp/agenthub-bazel-repo-cache-fresh --disk_cache=/tmp/agenthub-bazel-disk-cache-fresh --action_env=PATH="$PATH" --action_env=CARGO_HOME=/tmp/agenthub-cargo-home --action_env=RUSTUP_HOME=$HOME/.rustup //:ci_build
```
