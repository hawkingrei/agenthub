# Object Store S3 Release Enablement

## Summary

Official release and prebuild matrices now enable S3-compatible object storage through the existing
OpenDAL-backed `agenthub-object-store` abstraction. Runtime configuration still defaults to the
local filesystem and must explicitly select `backend = "s3"`. Main prebuild run
[`31259043337`](https://github.com/hawkingrei/agenthub/actions/runs/31259043337) closed the published
artifact gate: every supported matrix row succeeded, and the published Linux x86_64 binary passed a
MinIO smoke covering byte upload, hosted-image upload and public read, verification failure, and
compensating delete.

## Background

The OpenDAL S3 backend and MinIO fixture were feature-gated while release intent remained open. The
product decision is now to support S3 in official binaries without introducing a second S3 client
or making remote object storage the default backend.

## Scope

- Add the root `object-store-s3` feature to every `agenthub` release and prebuild matrix row.
- Keep root and object-store default features empty so local and Bazel builds remain unchanged.
- Keep runtime backend selection explicit and preserve `fs` as the default.
- Retain reviewed workflow and published-binary evidence for the release feature closure.
- Track the independently discovered Linux runtime baseline issue as release packaging work rather
  than treating it as an S3 backend failure.

## Key Decisions

- S3 data operations remain behind `AgentHubObjectStore` and `opendal::Operator`.
- Release workflows enable the root feature rather than the crate-internal `agenthub-object-store/s3`
  feature, preserving one public feature boundary.
- Release workflows keep an explicit feature list and continue to reject `--all-features`.
- MinIO is the compatibility fixture; AWS S3, Cloudflare R2, and other providers require their own
  production-certification evidence.
- Compiling S3 into the binary does not configure credentials, select an endpoint, or change the
  runtime backend.

## Published Artifact Evidence

The successful `push` run used main commit `a71ebe0975eb8891460f1a9730ac18fd01a98b8c` and completed
all supported rows:

- Linux x86_64 job `93106856351`
- Linux ARM64 job `93106856382`
- macOS ARM64 job `93106856376`

The Linux x86_64 log records this reviewed feature closure:

```text
cross build --locked --release --target x86_64-unknown-linux-gnu --bin agenthub \
  --features release-vendored-openssl,release-lance-fp16,rocksdb,object-store-s3
```

The smoke used the following immutable artifact evidence:

| Evidence | Value |
| --- | --- |
| Artifact ID | `9022659373` |
| Artifact name | `release-prebuild-x86_64-unknown-linux-gnu` |
| Artifact size | `337650437` bytes |
| GitHub artifact digest | `sha256:4300bede3c1ff01a841120ba360c84e8b23aed35a7a0b645a387c0a8f7436297` |
| Published archive | `agenthub-main-linux-amd64.tar.gz` |
| Published archive SHA-256 | `4438513d8a4298c30697dcf1d3e50f869640cb9cfb66930543e73ed627b3ce24` |
| Binary version | `agenthub 0.0.10` |
| Artifact expiry | `2026-08-15T13:52:48Z` |

The published binary ran in an Ubuntu 24.04 container with CA certificates against
`minio/minio:RELEASE.2025-06-13T11-33-47Z` at image digest
`sha256:064117214caceaa8d8a90ef7caa58f2b2aeb316b5156afe9ee8da5b4d83e12c8`.
Disposable credentials and an isolated bucket were used; no production provider was contacted.

| Runtime path | Evidence |
| --- | --- |
| Process health | `/health` returned `ok` |
| Byte upload | S3 metadata and downloaded bytes agreed on `9160` bytes and SHA-256 `569b0e18fd54c270370e43da36a43cc1261debdc67875d438725ea1b11900daf` |
| Hosted image | S3 metadata, the source image, and the public HTTP GET agreed on `10909` bytes and SHA-256 `7b645494cb372c3e67a823136d9a4eb907d9b6f9c47130ff9d0561796ec26a69` |
| Verification failure | An intentionally incorrect expected digest returned HTTP `400` with failure class `verification` |
| Compensating delete | `cleanup_attempts_total = 1`, `cleanup_successes_total = 1`, and `cleanup_failures_total = 0`; the failed object was absent from MinIO and its metadata row count was `0` |
| Retained successes | SQLite and MinIO contained exactly the two successful S3 objects totaling `20069` bytes |

This closes MinIO S3-compatibility evidence for the published Linux x86_64 artifact. It does not
certify AWS S3, Cloudflare R2, or another production provider.

The same artifact starts on Ubuntu 24.04 but does not start on Ubuntu 22.04 because it requires
`GLIBC_2.38` and `GLIBC_2.39`. That packaging baseline is tracked separately in `docs/todo.md`; it
does not invalidate the successful S3 path on a compatible runtime.

## Validation

```bash
cargo test official_release_includes_opendal_s3_without_changing_defaults --lib
cargo test -p agenthub-object-store --features s3 \
  s3_compatible_store_exercises_bytes_and_hosted_images -- --nocapture
cargo check --locked --features object-store-s3
cargo clippy --locked --workspace --all-targets --features object-store-s3 -- -D warnings
cargo fmt --all --check
git diff --check
bazel test --action_env=PROTOC=/opt/homebrew/bin/protoc \
  --test_arg=release_feature_tests::official_release_includes_opendal_s3_without_changing_defaults \
  //:agenthub_unit_tests
gh run view 31259043337 --job 93106856351 --log
gh api repos/hawkingrei/agenthub/actions/artifacts/9022659373
```

## Follow-Ups

- Certify each documented production provider separately before making provider-specific claims.
- Define and enforce the supported Linux glibc baseline, then add a published-binary startup smoke
  on the oldest supported Linux distribution.
