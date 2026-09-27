# Revoked Execution Sources

## Summary

Exclude revoked sources from live activation work reads. Independently coalesced work stays
executable, while revoked sources remain inspectable through activation history.

## Background

The [review on PR #1169](https://github.com/hawkingrei/agenthub/pull/1169#discussion_r4057730613)
identified that execution pages exposed revoked sources whenever another source kept the activation
alive. An agent could recover canceled work from that page or from an earlier source ID. The
integration PR is already merged, so this correction is a separate main-based follow-up.

## Scope

The two database execution-read queries, database and RPC regressions, the
[activation contract](../features/agent-loop-activation-contract.md), and a prerequisite repair to
the CI S3 fixture. Public response shapes,
revocation storage, history queries and the deferred native runtime track stay unchanged.

## Key Decisions

- Filter revocations in SQL before pagination, preserving full pages and accurate continuation.
- Recheck revocation on exact-source reads, including IDs cached before revocation.
- Keep stable ID cursors usable after their source is revoked.
- Retain source records and revocation markers in the authorized history surface.
- Restore the existing MinIO version using its official release package, pinned by SHA-256, after
  both container registries reject anonymous pulls. Keep both S3 fixture tests enabled.

## Validation

The regression uses five real schedule registrations coalesced into one admitted activation. It
revokes sources before, between and after live sources, then revokes the first live page's cursor.
It checks pagination, exact-source rejection, independent live work and complete history. Before
the fix, the first post-revocation page returned a revoked source and the regression failed.

Focused commands:

```sh
cargo test -p agenthub-db loop_work_context -- --nocapture
cargo test -p agenthub-db loop_ -- --nocapture
cargo clippy -p agenthub-db --all-targets -- -D warnings
cargo fmt --all --check
```

The fixed loop selection passes 71 tests, including the new regression. Database all-target Clippy
passes with warnings denied. Formatting and whitespace validation complete the local gate.

The initial full CI run also found an outdated RPC expectation in the signed App-event fixture:
it still required the revoked scheduled source to appear in live work. That fixture now requires
only the independent direct source in live work and `PermissionDenied` for the cached revoked ID,
while asserting both original revocation states through activation history. The Rust coverage and
Bazel root suites exercise this boundary in CI.

The first PR CI run failed before the S3 tests because Quay rejected the pinned MinIO image with
`unauthorized`; a scoped Docker Hub probe rejected the same tag as well. The
[official release](https://github.com/minio/minio/releases/tag/RELEASE.2025-06-13T11-33-47Z)
still provides `minio_20250613113347.0.0_amd64.deb`. Its published SHA-256 and the downloaded bytes
agree on `5a7157bb44a35ed5ff73cf676cc6bf1fec29671b82082810618005871eb31fa7`.
CI verifies that digest and extracts the package without installing it or running package scripts.

The extracted binary passed a temporary loopback smoke check for readiness, SigV4 bucket creation,
and a binary object upload/download round trip. Local extraction used `ar` and `tar` because
`dpkg-deb` is unavailable on the development host; the Ubuntu runner uses `dpkg-deb --extract`.
The changed workflow shell block passes `bash -n`. Both existing Rust S3 fixture tests remain the
remote acceptance gate.

## Follow-Ups

Complete the follow-up PR's current-head CI and review before merge. PR #1168 remains a deferred
draft; its incomplete native capabilities are separate from this ACP correction.
