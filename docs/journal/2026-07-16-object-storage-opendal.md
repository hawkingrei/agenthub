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

## Actor Upload Slice

The next slice adds the first agent-facing upload entry without exposing browser-direct uploads:

- `agenthub actor upload --file <path> --scope <owner_scope>` writes generic files through OpenDAL.
- `agenthub actor upload --file <path> --scope <owner_scope> --image` writes allowlisted raster
  images through the image-hosting helper.
- `object_uploads` stores backend, object key, original filename, MIME type, size, SHA-256, owner
  scope, publishing actor, public URL, publish state, and cleanup timestamps.
- Upload code is split by responsibility: actor CLI upload orchestration lives under
  `src/actor_cli/upload.rs`, upload metadata helpers live under `agenthub-db::object_uploads`, and
  `agenthub-object-store` remains the byte-storage boundary.
- Local filesystem uploads default to `~/.agenthub/objects` when `[object_store].root` is omitted.
- Metadata publication failure triggers a best-effort delete of the just-written object.

Focused checks for this slice:

```bash
cargo test -p agenthub-db insert_object_upload_persists_published_metadata
cargo test -p agenthub parse_upload
cargo test -p agenthub actor_output_preference_contract_covers_all_command_variants
cargo fmt --all --check
```
