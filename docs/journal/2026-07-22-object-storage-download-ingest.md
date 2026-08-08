# Object Storage Download Ingest Decision

## Summary

The large-object path should be server-side download ingestion. Browser
JSON/base64 uploads remain the small-object path, while large files enter
AgentHub by submitting a download intent that AgentHub fetches, verifies,
stores, and publishes under route-derived ownership.

## Background

The previous TODO framed the decision as browser-facing multipart versus
presigned upload tokens after Team, task, and agent JSON/base64 owner-scope
routes. The clarified product shape is different: large files are external
downloads, not browser-originated large request bodies.

## Scope

- Define download ingestion as the canonical large-object path.
- Keep browser multipart and presigned upload URLs out of the default product
  path.
- Preserve existing owner-scope rules: API routes derive Team, task, or agent
  ownership and never accept raw browser owner scopes.
- Keep SQLite metadata as the publication and authorization authority.

## Key Decisions

- The browser submits a download intent with source URL, file name, content
  type, optional expected size, optional expected SHA-256, and optional image
  mode.
- AgentHub performs the remote fetch on the server, streams bytes into object
  storage, computes SHA-256 while streaming, verifies expected metadata, and
  publishes the `object_uploads` row only after successful verification.
- Download intents must fail closed for URL scheme, DNS/address class, redirects,
  timeout, byte limit, checksum mismatch, and metadata publication failure.
- AgentHub must not forward browser cookies, AgentHub credentials,
  authorization headers, or ambient provider secrets to the remote URL.
- S3 multipart and presigned uploads remain possible backend transport tools
  for a future browser-upload path, not the canonical external-file ingest path.

## Validation

The first synchronous implementation landed in
[2026-07-23-object-storage-download-ingest-implementation.md](2026-07-23-object-storage-download-ingest-implementation.md).
Its focused checks are:

```bash
cargo test -p agenthub-object-store -- --nocapture
cargo test -p agenthub download -- --nocapture
cargo test -p agenthub agent_upload_routes_publish_agent_scoped_metadata -- --nocapture
cargo test -p agenthub openapi -- --nocapture
cargo test -p agenthub-config object_store -- --nocapture
cargo fmt --all --check
git diff --check
```

## Follow-Ups

- Source host policy, retry, per-host concurrency, structured logs, and durable terminal metrics
  landed in the implementation checkpoints linked above and in
  [Object Storage Download Observability](2026-08-08-object-storage-download-observability.md).
- Add an async intent table only if product flows require queued/cancelable downloads or durable
  in-progress state.
