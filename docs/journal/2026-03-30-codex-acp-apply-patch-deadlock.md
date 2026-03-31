# 2026-03-30 Codex ACP Apply Patch Deadlock

## Context

`agenthub-codex-acp` still ran the ACP agent-side I/O loop on the same local Tokio
execution lane as tool handling. That is normally fine for lightweight ACP request
handling, but it breaks when a tool synchronously calls back into ACP-backed
filesystem reads.

The concrete risk showed up on the `apply_patch` path:

1. `codex_apply_patch::maybe_parse_apply_patch_verified(...)` verifies a patch
   against the current file contents.
2. Under `agenthub-codex-acp`, existing file contents may come from `AcpFs`
   rather than the local filesystem.
3. `AcpFs::read_to_string()` issues an ACP `read_text_file` round-trip and waits
   synchronously for the reply.
4. If the ACP I/O loop shares the same execution lane as the tool call, the tool
   can end up waiting on work that the runtime is no longer driving.

That leaves the ACP session stuck at the tool boundary with no patch applied.

## Upstream Reference

- `zed-industries/codex-acp#214`
- title: `fix: avoid ACP apply_patch deadlock`

AgentHub vendors `codex-acp` under `agenthub-codex-acp/`, so the correct move is
to backport the narrow runtime/threading fix into the vendored adapter instead of
waiting for a future dependency refresh.

## Changes

1. Added `spawn_acp_io_task(...)` in `agenthub-codex-acp/src/lib.rs`.
   - runs ACP agent-side I/O on a dedicated OS thread
   - builds a dedicated current-thread Tokio runtime for that thread
   - returns a `oneshot` receiver so the main task can still await terminal I/O
     completion and surface a clean failure if the ACP I/O thread dies early

2. Updated `run_main(...)` in `agenthub-codex-acp/src/lib.rs`.
   - keep the main `LocalSet` for agent/tool execution
   - move `AgentSideConnection::new(...).io_task` onto the dedicated ACP I/O
     thread instead of awaiting it directly on the same `LocalSet`

3. Added a deterministic regression test in
   `agenthub-codex-acp/src/local_spawner.rs`.
   - creates a fake ACP client/server pair over pipes
   - serves file contents through `AcpFs`
   - calls `codex_apply_patch::maybe_parse_apply_patch_verified(...)` on an
     update patch against an existing file
   - fails the parent test if the child process does not finish within 5 seconds

4. Added `piper` as a test-only dependency for the ACP pipe harness.

## Validation

- `cargo fmt --manifest-path agenthub-codex-acp/Cargo.toml`
- `cargo test -p agenthub-codex-acp apply_patch_verification_does_not_deadlock_over_acp_fs -- --nocapture`
- `cargo test -p agenthub-codex-acp --lib`
- `cargo clippy --locked -p agenthub-codex-acp --all-targets -- -D warnings`
- `git diff --check`

## Result

`agenthub-codex-acp` no longer relies on a single local execution lane for both:

- tool execution
- ACP request/response progress

That removes the deadlock class where ACP-backed filesystem verification waits on
an ACP reply that the runtime is no longer able to drive.

## Verified Evidence

- Focused regression coverage stayed on:
  - `cargo test -p agenthub-codex-acp apply_patch_verification_does_not_deadlock_over_acp_fs -- --nocapture`
  - `cargo test -p agenthub-codex-acp --lib`
  - `cargo clippy --locked -p agenthub-codex-acp --all-targets -- -D warnings`
- `pull_request` CI for PR `#259`:
  - Bazel: `23745923494`
  - Rust: `23745923506`
  - Clippy: `23745923527`
  - Web: `23745923465`
  - Web E2E: `23745923480`
  - User Docs: `23745923469`
  - Distributed P2P Pipeline: `23745923471`
- Subsequent default-branch `push` verification on commit `4b871fca`:
  - Bazel: `23775356172`
  - Rust: `23775356140`
  - Clippy: `23775356149`
  - Web: `23775356151`
  - Web E2E: `23775356147`
  - User Docs: `23775356152`
  - Distributed P2P Pipeline: `23775356155`
