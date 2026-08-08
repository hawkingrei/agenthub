# Object Store S3 Release Enablement

## Summary

Official release and prebuild matrices now enable S3-compatible object storage through the existing
OpenDAL-backed `agenthub-object-store` abstraction. Runtime configuration still defaults to the
local filesystem and must explicitly select `backend = "s3"`; successful published artifacts remain
a separate rollout gate.

## Background

The OpenDAL S3 backend and MinIO fixture were feature-gated while release intent remained open. The
product decision is now to support S3 in official binaries without introducing a second S3 client
or making remote object storage the default backend.

## Scope

- Add the root `object-store-s3` feature to every `agenthub` release and prebuild matrix row.
- Keep root and object-store default features empty so local and Bazel builds remain unchanged.
- Keep runtime backend selection explicit and preserve `fs` as the default.
- Update the release feature regression test, canonical spec, and artifact-validation TODO.

## Key Decisions

- S3 data operations remain behind `AgentHubObjectStore` and `opendal::Operator`.
- Release workflows enable the root feature rather than the crate-internal `agenthub-object-store/s3`
  feature, preserving one public feature boundary.
- Release workflows keep an explicit feature list and continue to reject `--all-features`.
- MinIO is the compatibility fixture; AWS S3, Cloudflare R2, and other providers require their own
  production-certification evidence.
- Compiling S3 into the binary does not configure credentials, select an endpoint, or change the
  runtime backend.

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
```

## Follow-Ups

- Record one reviewed preview or semver release whose artifact matrix compiles the explicit S3
  feature on every supported target.
- Run the published Linux x86_64 binary against MinIO and retain byte, hosted-image, and delete
  evidence.
- Certify each documented production provider separately before making provider-specific claims.
