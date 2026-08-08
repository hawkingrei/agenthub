# Object Storage Download Observability

## Summary

Server-side download ingestion now records durable, low-cardinality terminal metrics in SQLite.
The counters survive process restarts and distinguish successful publication, bounded failure
classes, downloaded bytes, latency, and compensation cleanup outcomes.

## Background

Download ingestion already enforced source policy, bounded pre-stream retries, per-host
concurrency, streaming size limits, checksum verification, and structured logs. Those logs were
not durable, and the success log was emitted after the object write but before checksum
verification and metadata publication. A later verification or SQLite failure could therefore be
reported as both a success and a failure.

## Scope

- Add backend-scoped cumulative counters in `object_download_metrics`.
- Add bounded failure-class counters in `object_download_failure_metrics`.
- Record attempts, successes, failures, downloaded bytes, latency sum/max, and cleanup
  attempt/success/failure totals.
- Count request validation, source/request, verification, and metadata publication terminal
  failures without persisting source hosts, URLs, owner scopes, or credentials.
- Emit the terminal success log only after final verification and `object_uploads` publication.
- Keep metric persistence observational: a metric write failure emits a warning but does not turn
  an already-published object into an ambiguous client-visible failure.

## Key Decisions

- Use monotonic SQLite aggregates rather than process-local atomics so restart does not reset the
  evidence.
- Use fixed failure classes rather than arbitrary error text or per-source labels, preserving
  bounded cardinality and avoiding sensitive metadata in the metric tables.
- Count bytes once the object-store stream returns a complete object, including objects later
  rejected by verification or metadata publication. Preflight and pre-stream failures contribute
  zero downloaded bytes.
- Track compensation cleanup separately from the primary failure class. Cleanup failure must not
  hide whether the primary failure came from verification or metadata publication.
- Do not add a queued intent table until a product flow requires queued, cancelable, expiring, or
  durably inspectable in-progress downloads.

## Validation

```bash
cargo test -p agenthub-db object_download_metrics_accumulate_terminal_and_cleanup_outcomes -- --nocapture
cargo test -p agenthub team_download_api_streams_remote_object_and_publishes_metadata -- --nocapture
cargo test -p agenthub download_ -- --nocapture
cargo fmt --all --check
cargo clippy --locked -p agenthub-db -p agenthub --all-targets -- -D warnings
bazel test --action_env=PROTOC=/opt/homebrew/bin/protoc \
  --test_arg=object_download_metrics_accumulate_terminal_and_cleanup_outcomes \
  //crates/agenthub-db:agenthub_db_tests
bazel test --action_env=PROTOC=/opt/homebrew/bin/protoc \
  --test_arg=team_download_api_streams_remote_object_and_publishes_metadata \
  //:agenthub_unit_tests
git diff --check
```

The focused service regression covers successful publication, checksum rejection, request
validation rejection, forced metadata publication failure, cleanup accounting, bounded failure
classes, and the invariant that failed downloads do not publish additional metadata rows.

## Follow-Ups

- Add a durable async intent table only if a product surface introduces queued/cancelable downloads
  or needs durable in-progress and terminal failure state.
- Any future operator-facing metrics exporter should read these aggregates without adding
  source-host, URL, actor, or owner-scope labels.
