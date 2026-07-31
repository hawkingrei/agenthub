# Object Store S3 MinIO Fixture

## Summary

This checkpoint adds the first S3-compatible integration fixture for the OpenDAL object-store
backend. CI now starts MinIO, creates a fixture bucket, and runs the `agenthub-object-store` tests
with the `s3` feature enabled. The fixture now also compiles and exercises the low-level direct
multipart primitive path when S3 credentials are available, and the same CI job now runs the root
API route fixture with `object-store-s3` enabled.

## Background

The object-store foundation compiled S3 support behind an opt-in feature, but only local filesystem
tests exercised byte storage and image-hosting behavior. Before enabling S3 in release builds, the
backend needs a real S3-compatible endpoint fixture that verifies AgentHub key, prefix, checksum,
read/delete, hosted-image URL behavior through OpenDAL, and the direct multipart REST shape used by
future upload-session browser transports.

## Scope

- Add an env-gated S3 compatibility test to `agenthub-object-store`.
- Add a Rust CI job that runs MinIO in Docker and creates the fixture bucket with `mc`.
- Exercise low-level multipart initiate, upload-part presign, complete, and abort primitives in the
  same S3 fixture.
- Exercise Team upload-session HTTP multipart initiate, presigned part upload, complete, abort, and
  metadata publication through the API route fixture.
- Keep the S3 feature out of default workspace, Bazel, and release feature sets, with a local
  manifest/workflow guard that fails if release workflows enable S3 before review.
- Tighten the local release-feature guard so release workflows cannot use `--all-features` while S3
  remains opt-in, and so the active TODO must continue requiring reviewed release-build intent.
- Keep provider-specific production certification out of this slice.

## Key Decisions

- The test skips when `AGENTHUB_OBJECT_STORE_S3_TEST_*` is unset, so local default test runs do not
  require object-store credentials.
- CI runs the S3 fixtures explicitly with `cargo test -p agenthub-object-store --features s3` and
  `cargo test --features object-store-s3`.
- MinIO is the first compatibility fixture, not proof that every S3-compatible provider is
  production-ready.

## Validation

```bash
cargo test -p agenthub-object-store --features s3 s3_compatible_store_exercises_bytes_and_hosted_images -- --nocapture
cargo test --features object-store-s3 team_upload_session_s3_multipart_route_fixture_publishes_metadata -- --nocapture
cargo test object_store_s3_stays_out_of_default_and_release_feature_sets --locked
cargo test object_store_s3_stays_out_of_default_and_release_feature_sets --lib -- --nocapture
gh pr view 890 --json number,state,mergedAt,headRefName,baseRefName,title,url,statusCheckRollup,reviewDecision
gh run view 29639782907 --json databaseId,status,conclusion,workflowName,headBranch,headSha,jobs
```

CI evidence:

- PR `#890` (`test(ci): add object-store s3 minio fixture`) merged into `main` at
  `2026-07-18T09:48:32Z`.
- PR check `Rust (Object Store S3 MinIO)` completed successfully.
- Main push Rust workflow run `29639782907` completed successfully for
  `a499de855662d5cfd6ea345d68c6122d481dfcba`.
- Main push job `Rust (Object Store S3 MinIO)` / `88068255089` completed successfully after:
  - `Start MinIO`
  - `Create fixture bucket`
  - `Cargo test object store S3 fixture`

## Follow-Ups

- Keep `agenthub-object-store/s3` out of release feature sets until one reviewed release build
  includes the feature intentionally.
