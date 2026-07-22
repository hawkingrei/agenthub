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

Planned focused checks for the implementation slice:

```bash
cargo test -p agenthub object_download
cargo test -p agenthub openapi_json_contains_download_ingest_routes
cargo fmt --all --check
git diff --check
```

## Follow-Ups

- Implement download intent metadata and server-side streaming ingest.
- Add operator config for maximum bytes, source host allow/deny policy,
  redirect limits, timeout limits, and retry policy.
- Add observability for download latency, bytes written, failure class, cleanup
  compensation, and publish success.
