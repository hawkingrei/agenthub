# Daemon Instance Ownership Fencing

## Summary

Added an exclusive daemon ownership boundary keyed by canonical SQLite database path and effective
node ID. Each owner now claims a durable generation before startup mutations, and shutdown cleanup
refuses to publish state from a stale generation.

## Scope

- Added an operating-system advisory lock held for the daemon lifetime.
- Added diagnostic lock metadata without credentials or configuration secrets.
- Added the `daemon_generations` SQLite table and atomic per-node claim operation.
- Reordered daemon initialization so generation claim precedes root creation and startup cleanup.
- Added a generation check before server role startup and before shutdown cleanup.
- Added focused lock-scope, canonical-path, release, metadata, increment, and stale-owner tests.

## Key Decisions

- Scope ownership by canonical database path plus node ID instead of listen address. Database-backed
  recovery and runtime state are the protected authority.
- Keep the lock file after release and hold the lock through the open file description. Removing a lock
  file can let another process lock a replacement inode while an old owner is still active.
- Use a full SHA-256 digest in the lock filename so arbitrary node IDs never become filesystem paths.
- Use the operating-system lock as the primary coexistence guard and the database generation as a
  durable fencing token for lifecycle writes.
- Claim the generation inside daemon-specific state initialization, after schema setup but before any
  application mutation or worker startup.

## Validation

The focused gates pass:

```bash
cargo fmt --all -- --check
cargo check -p agenthub --lib
cargo clippy -p agenthub --lib -- -D warnings
cargo test -p agenthub daemon_instance::tests -- --nocapture
cargo test -p agenthub-db daemon_generation -- --nocapture
bazel test //crates/agenthub-db:agenthub_db_tests --test_output=errors
git diff --check
```

The daemon lock suite has three passing tests. The database generation suite has one passing test that
covers independent node sequences, atomic replacement, current-owner verification, and stale-owner
rejection. The full `agenthub-db` Cargo and Bazel suites both pass with 51 tests. The existing main/node
root-user setup regressions also pass after the initialization ordering change.

The broader `cargo test -p agenthub --lib` run completed with 780 passing tests and two failures. Both
failures are the existing `state::tests::initialize_services_*` panic in
`lance-namespace-impls 8.0.0`'s directory-manifest old-version-cleanup assertion; neither reaches the
daemon ownership code.

`bazel build //:agenthub_lib` stops before compiling the root crate because the existing
`lance-datafusion 8.0.0` build script cannot find `protoc` inside the local Bazel sandbox. The focused
`agenthub-db` Bazel target proves the new schema module under Bazel. No Bazel configuration was
changed.

## Follow-Ups

- Validate the exact change head through Bazel CI and supported release platforms.
- Add the global agent-start scheduler, durable mailbox delivery receipts, and unified daemon task
  shutdown as separate reviewable runtime slices.
