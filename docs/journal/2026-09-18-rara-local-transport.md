# Direct Runtime Transport

## Summary

Slice 16 starts with a dedicated configuration and bounded protocol/connection crate. Its
fixtures come from the actual independently tested upstream child process at
`6f489462251b73e1695bb22a59d2ece59ba26a21`, whose prerequisite PR has eight passing
CI checks. This checkpoint does not enable managed provider execution or complete
the 18-slice implementation plan.

## Background

The existing provider path selects ACP or plain stdin. Direct runtime control
requires a distinct structured connection; ordinary text cannot be sent to its
stdin and event provenance alone cannot identify the owned session. The process
supervisor remains the owner of child lifetime and descendant cleanup.

## Scope

- Separate `[rara]` configuration, explicit environment overrides and bounded
  startup/shutdown timeouts, without copying runtime credentials.
- Fixed app-server argv, explicit workspace and safe provider/model value boundaries.
- Versioned handshake validation, concrete method requirements and explicit
  receipt/replay/approval lifetimes.
- Session-owned event envelopes, original canonical identities, correlated ACKs,
  replay gaps and semantic shutdown vocabulary.
- Bounded LF/CRLF framing and cancellation-safe partial reads.
- Independent ordered writes and reads, bounded queues/receipts, request deadlines,
  explicit consumer failure and correlated shutdown completion followed by EOF.
- Cargo and Bazel crate targets using existing dependencies.

## Key Decisions

The crate owns wire types and streams, not processes or a second task/history
store. It does not import the upstream runtime library. The protocol pin is a
tested source revision and captured fixture, not only a package version label.
Unknown optional method/family identifiers do not grant capabilities; required
methods must be present. Runtime-only replay and receipts cannot prove continuity
across process replacement.

## Validation

Focused checks for this checkpoint:

```bash
cargo test --locked --offline -p agenthub-rara -p agenthub-config --lib
cargo clippy --locked --offline -p agenthub-rara -p agenthub-config --all-targets -- -D warnings
cargo fmt --all --check
git diff --check
```

The configuration suite has 44 passing tests. The protocol/connection crate has
20 passing default tests and one opt-in native process test. The native process
test also passes against the pinned binary with SHA-256
`08f6e9b90704b60491efd6115acee7bd7a6c8c5a27cf80c2ee12f51002f190b4`.
It uses isolated native configuration and workspace, creates a session, consumes
its owned event, completes semantic shutdown with stdin open and verifies process
exit. It makes no model request and does not establish supervised-manager cleanup.
The crate/config all-target Clippy check passes with warnings denied.

Run that explicit native check only with the compatible source build:

```bash
AGENTHUB_RARA_TEST_BINARY=/absolute/path/to/pinned/binary \
  cargo test --locked --offline -p agenthub-rara native_process_transport_round_trip -- --ignored
```

Tests cover the real captured frames, session ownership with null provenance,
method/lifetime mismatches, cancellation after a partial read, inclusive frame
limits, invalid input, safe errors, explicit turn targets and configuration/argv
boundaries. Connection tests cover startup failure, unknown ACK outcomes, caller
cancellation, receipt exhaustion, blocked writes, uncorrelated frames, consumer
loss/stalls and missing/wrong/trailing shutdown frames. Supervised-manager evidence
will be added before slice 16 is delivered.

## Follow-Ups

Implement supervised local launch/cleanup in this slice. Durable ACK/event mapping
belongs to slice 17; role/source and loop outcome alignment belongs to slice 18. The canonical contract is
[Rara Direct Integration](../features/rara-direct-integration.md).
