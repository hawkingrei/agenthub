# Direct Runtime Transport

## Summary

Slice 16 adds dedicated configuration, a bounded protocol/connection crate and
managed local launch/cleanup through the existing executor and supervisor. Its
fixtures come from the actual independently tested upstream child process at
`6f489462251b73e1695bb22a59d2ece59ba26a21`, whose prerequisite PR has eight passing
CI checks. Durable request/event mapping and loop admission remain separate slices;
this change does not complete the 18-slice implementation plan.

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
- A distinct managed handle, startup stderr drain, failed-start cleanup, semantic
  stop and daemon shutdown followed by supervised process-group cleanup.
- Explicit input, Team, remote and loop gates until their mapping contracts exist.

## Key Decisions

The crate owns wire types and streams, not processes or a second task/history
store. It does not import the upstream runtime library. The protocol pin is a
tested source revision and captured fixture, not only a package version label.
Unknown optional method/family identifiers do not grant capabilities; required
methods must be present. Runtime-only replay and receipts cannot prove continuity
across process replacement.

Manager regression exposed an existing nullable-exit bug: mapping a present row
directly to an integer treated SQL `NULL` as an already completed session. Exit
finalization now reads the optional timestamp correctly. A late exit may update
its own session but cannot overwrite a newer launch's agent status, even when both
started in the same second. Both changes have focused regression coverage.

## Validation

Focused checks for this checkpoint:

```bash
cargo test --locked --offline -p agenthub-rara -p agenthub-config --lib
cargo clippy --locked --offline -p agenthub-rara -p agenthub-config --all-targets -- -D warnings
cargo test --locked --offline -p agenthub --lib agent::manager:: -- --test-threads=1
cargo clippy --locked --offline -p agenthub --all-targets -- -D warnings
cargo fmt --all --check
git diff --check
```

The configuration suite has 44 passing tests. The protocol/connection crate has
20 passing default tests and one opt-in native process test. The native process
test also passes against the pinned binary with SHA-256
`08f6e9b90704b60491efd6115acee7bd7a6c8c5a27cf80c2ee12f51002f190b4`.
It uses isolated native configuration and workspace, creates a session, consumes
its owned event, completes semantic shutdown with stdin open and verifies process
exit. A second opt-in native check starts and stops that same build through the
real manager, local executor and supervisor, then verifies terminal session state
and absence of a tracked child. Neither probe makes a model request.

The manager regression suite has 121 passing tests and six intentionally ignored
fixtures, including the separately executed native check. Existing ACP, ordinary
command, scheduling, Mem and App workflows pass alongside the new lifecycle tests.
The first broader attempt encountered sandbox restrictions on localhost listeners
and process signals; the complete suite passed with those local test permissions.
Root, crate and configuration all-target Clippy checks pass with warnings denied.

Run that explicit native check only with the compatible source build:

```bash
AGENTHUB_RARA_TEST_BINARY=/absolute/path/to/pinned/binary \
  cargo test --locked --offline -p agenthub-rara native_process_transport_round_trip -- --ignored
AGENTHUB_RARA_TEST_BINARY=/absolute/path/to/pinned/binary \
  cargo test --locked --offline -p agenthub --lib managed_native_process_transport -- --ignored
```

Tests cover the real captured frames, session ownership with null provenance,
method/lifetime mismatches, cancellation after a partial read, inclusive frame
limits, invalid input, safe errors, explicit turn targets and configuration/argv
boundaries. Connection tests cover startup failure, unknown ACK outcomes, caller
cancellation, receipt exhaustion, blocked writes, uncorrelated frames, consumer
loss/stalls and missing/wrong/trailing shutdown frames. Manager tests cover a large
stderr burst without diagnostic persistence, handshake failure/timeout cleanup,
concurrent semantic stop, stalled drain with a descendant, daemon shutdown,
exit-code-zero transport failure and pre-spawn placement/argument rejection.

[PR #1163](https://github.com/hawkingrei/agenthub/pull/1163) is ready for review at
`e6b0cd6cc32736a6139356aa8c46d61ef871c7e5`. All applicable CI checks passed, including
Cargo, Clippy, both coverage jobs, Bazel build/root/crate tests, distributed P2P,
protocol, S3, RocksDB, documentation and Web/E2E. Conditional gRPC integration was
skipped by its workflow. Codecov's patch/project gates pass with 92.27% patch coverage.
Reviews and inline comments contain no outstanding requests. No merge was performed.

## Follow-Ups

Durable ACK/event mapping belongs to slice 17; role/source and loop outcome alignment
belongs to slice 18. The canonical contract is
[Rara Direct Integration](../features/rara-direct-integration.md).
