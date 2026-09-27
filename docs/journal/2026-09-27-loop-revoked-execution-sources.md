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

The two database execution-read queries, one focused regression, and the
[activation contract](../features/agent-loop-activation-contract.md). Public response shapes,
revocation storage, history queries and the deferred native runtime track stay unchanged.

## Key Decisions

- Filter revocations in SQL before pagination, preserving full pages and accurate continuation.
- Recheck revocation on exact-source reads, including IDs cached before revocation.
- Keep stable ID cursors usable after their source is revoked.
- Retain source records and revocation markers in the authorized history surface.

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

## Follow-Ups

Complete the follow-up PR's current-head CI and review before merge. PR #1168 remains a deferred
draft; its incomplete native capabilities are separate from this ACP correction.
