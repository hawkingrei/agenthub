# Native Runtime Qualification

## Summary

Pin the consumer to the assembled native candidate containing both the complete runtime
capabilities and the terminal-input CI prerequisite. Recapture the protocol from that exact
executable and qualify the combined path with isolated local fixtures.

## Background

The prior native candidate and terminal correction were qualified on separate producer branches.
The terminal regression can discard a queued key when resize and input readiness arrive together.
An ordinary merge preserves both branches and gives publication one candidate to review and test.

## Scope

Update the pinned producer revision, captured executable identity and qualification evidence.
The candidate identity is:

- Producer commit: `df7ed52b684aa175cb1f280ea306abaafedf73e2`.
- Native parent: `178dfecf6599ad536f53f5f207d37d547b510a6b`.
- Terminal prerequisite: `e728b458d3bd6dcb778b9e94df2cf3a60045936d`.
- Executable SHA-256: `dc82a0c5eca3dd464faa3a2c4ec36fae1cda629b91b2b51b9fd10adc1525c283`.
- Package version: `0.0.22`; this version alone does not identify capabilities.

## Key Decisions

The executable was built from the clean merged producer checkout, copied to a private validation
archive and stripped. The previous qualified archive and installed executable are preserved.
The six captured protocol frames are unchanged; only the fixture's producer revision and binary
hash differ. Consumer behavior and the wire contract are unchanged.

## Validation

Producer Cargo checks cover 49 runtime-session tests, 21 stdio tests, 25 protocol-crate tests
and both active external-editor PTY regressions. All pass. Five actual-process smoke programs
cover 26 scenarios: normal/control and transport failure paths, semantic outcomes, conversation
continuity, durable decisions and guarded reentry. All use isolated localhost model fixtures.

The consumer protocol crate passes 58 tests, with its opt-in process test left ignored.
All six prepared Team entrypoints pass against this exact executable: semantic outcomes,
controlled App/Mem, nested lifecycle, conversation recall, restored approvals and uncertain-effect
review. The run makes 42 localhost model requests, including 14 tool-free semantic/reentry checks,
and no external provider requests. Its production control CLI, proxies, native processes and
database/manager reconstruction remain active parts of the checks.

Relevant commands include:

```bash
cargo test -p agenthub-rara
cargo test -p agenthub --lib native_process::configured:: -- --ignored --test-threads=1
cargo test -p agenthub --lib native_process::continuity::configured:: -- --ignored --test-threads=1
bazel test //:rara_unit_tests --test_arg=tui::external_editor::pty_tests --test_output=errors
```

The Team commands require the explicit candidate and private localhost provider configuration.
The separate controlled-tool parent entrypoint is listed in the prepared acceptance journal below.
The Bazel command runs in the producer checkout with default configuration and passes both PTY
cases. Producer all-target Clippy and consumer protocol-crate Clippy pass with warnings denied;
formatting, whitespace and 136 local documentation links pass. The owned model service stopped,
and the reused worktree completed `bazel clean` and `bazel shutdown`. Generated Bazel lockfile
cache changes were discarded; no Bazel configuration changed.

Local logs use `/tmp/agenthub-native-assembled-*.log`. The private binary archive is
`target/loop-review-validation/qualified-native/rara-assembled-candidate`; request evidence and
the bounded payload audit are under `target/loop-review-validation/native-assembled-loopback-20261007`.

## Follow-Ups

The producer merge is local and has not been published. Its exact commit must become available
upstream and pass applicable CI before this pin is ready for integration. The installed runtime
still lacks required negotiated capabilities and has not been replaced. External configured-provider
Team acceptance also remains pending; localhost results do not satisfy that gate.

Contract: [direct runtime integration](../features/rara-direct-integration.md).
Prepared acceptance: [native Team provider checks](2026-10-07-native-team-provider-acceptance.md).
Active follow-up: [TODO](../todo.md).
