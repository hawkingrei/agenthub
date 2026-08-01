# Object Store S3 MinIO Fixture

## Summary

This checkpoint adds the first S3-compatible integration fixture for the OpenDAL object-store
backend. CI now starts MinIO, creates a fixture bucket, and runs the `agenthub-object-store` tests
with the `s3` feature enabled.

## Background

The object-store foundation compiled S3 support behind an opt-in feature, but only local filesystem
tests exercised byte storage and image-hosting behavior. Before enabling S3 in release builds, the
backend needs a real S3-compatible endpoint fixture that verifies AgentHub key, prefix, checksum,
read/delete, and hosted-image URL behavior through OpenDAL.

## Scope

- Add an env-gated S3 compatibility test to `agenthub-object-store`.
- Add a Rust CI job that runs MinIO in Docker and creates the fixture bucket with `mc`.
- Keep the S3 feature out of default workspace, Bazel, and release feature sets.
- Keep provider-specific production certification out of this slice.

## Key Decisions

- The test skips when `AGENTHUB_OBJECT_STORE_S3_TEST_*` is unset, so local default test runs do not
  require object-store credentials.
- CI runs the test explicitly with `cargo test -p agenthub-object-store --features s3`.
- MinIO is the first compatibility fixture, not proof that every S3-compatible provider is
  production-ready.

## Validation

```bash
cargo test -p agenthub-object-store --features s3 s3_compatible_store_writes_reads_and_deletes_bytes -- --nocapture
```

### 2026-07-29 Release Feature Gate

Added a local regression test for the release intent boundary:

- root `default` remains empty;
- root `object-store-s3` remains the only bridge to `agenthub-object-store/s3`;
- `release-vendored-openssl`, `release-lance-fp16`, and `rocksdb` do not imply S3;
- `agenthub-object-store` keeps `default = []` and keeps S3 behind
  `opendal/http-transport-reqwest` plus `opendal/services-s3`;
- `release.yml` and `release-prebuild.yml` do not use `--all-features` and do not list
  `object-store-s3` or `agenthub-object-store/s3`.

Validation:

```bash
cargo test -p agenthub --test release_feature_sets -- --nocapture
cargo fmt --all --check
```

## Follow-Ups

- Keep `agenthub-object-store/s3` out of release feature sets until one reviewed release build
  intentionally includes it. The local release feature gate now protects against accidental default,
  release, prebuild, or `--all-features` enablement, but it is not a substitute for the future release
  intent review.
- Decide whether multipart or presigned upload tokens are the canonical large-object browser path.
