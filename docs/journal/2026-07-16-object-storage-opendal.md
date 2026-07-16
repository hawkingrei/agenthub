# Object Storage OpenDAL Foundation

## Summary

Added the first AgentHub object storage foundation around OpenDAL. The slice introduces a small Rust
crate for normalized object keys, local filesystem writes, size/checksum reporting, and optional
S3-compatible construction behind a feature gate. It also adds the first constrained image-hosting
helper for future graph-bed style uploads.

## Background

AgentHub needs file upload and artifact storage that can start local and later target AWS S3 or
S3-compatible services. The design follows the existing AgentHub storage boundary: SQLite remains
the metadata/control plane, while object storage stores bytes only.

## Scope

- New `agenthub-object-store` crate.
- Secret-free `[object_store]` config accessors in `agenthub-config`.
- Optional `public_base_url` config for deployments that front object bytes through a CDN or object
  gateway.
- Bazel test and coverage target registration for the new crate.
- Canonical feature spec for object storage contracts and rollout risks.

## Key Decisions

- Keep OpenDAL behind a focused crate instead of letting upload handlers build operators directly.
- Compile S3 support behind the crate `s3` feature while keeping local `fs` available by default.
- Reject unsafe object keys before they reach a backend.
- Treat graph-bed uploads as a constrained image-hosting consumer: server-scoped keys, raster MIME
  allowlist, checksum/size recording, and optional public URL derivation.
- Treat object store writes as unpublished until DB metadata verifies and links size/checksum.
- Defer Team context artifact migration because its current table stores absolute local paths.

## Validation

Planned focused checks:

```bash
cargo test -p agenthub-object-store
cargo test -p agenthub-config object_store
cargo check -p agenthub-object-store --features s3
cargo fmt --all --check
```

## Follow-Ups

- Wire the object store into runtime application state with a default `~/.agenthub/objects` local
  root.
- Add a metadata schema for uploaded files that stores backend, object key, size, checksum, MIME
  type, owner scope, publish state, and cleanup timestamps.
- Add image-hosting API routes for graph-bed uploads after the metadata schema exists.
- Add an S3-compatible integration fixture before enabling the S3 feature in release builds.
