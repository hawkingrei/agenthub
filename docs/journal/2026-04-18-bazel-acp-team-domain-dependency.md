# Bazel ACP Team Domain Dependency

- Date: 2026-04-18
- PR: #383

## Summary

The Team memory/index compatibility change added a new Rust dependency from `agenthub-acp` to `agenthub-team-domain`, but the Bazel target graph still reflected the old crate boundary. Cargo builds passed because `crates/agenthub-acp/Cargo.toml` declared the path dependency, while Bazel CI failed with an unresolved import for `agenthub_team_domain`.

## Root Cause

- `crates/agenthub-acp/src/actor_runtime_skill.rs` now imports shared Team-domain helpers.
- `crates/agenthub-acp/Cargo.toml` was updated accordingly.
- `crates/agenthub-acp/BUILD.bazel` was not updated, so `//crates/agenthub-acp:agenthub_acp` and its test target did not depend on `//crates/agenthub-team-domain:agenthub_team_domain`.

## Fix

- Add `//crates/agenthub-team-domain:agenthub_team_domain` to the `agenthub_acp` Bazel library target.
- Add the same explicit dependency to `agenthub_acp_tests` so Bazel test builds stay aligned with the library boundary.

## Validation

- GitHub Actions failure logs for PR #383 showed the same unresolved-import error across Bazel Build/Test/Coverage jobs.
- Local `bazel build/test` attempts no longer reproduced the unresolved-import error, but local Bazel execution remained noisy because of workstation-specific output-base/runtime issues, so the authoritative verification is the PR Bazel rerun on CI.
