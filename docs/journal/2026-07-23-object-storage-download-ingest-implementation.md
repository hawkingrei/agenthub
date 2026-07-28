# Object Storage Download Ingest Implementation

## Summary

AgentHub now exposes the first synchronous server-side download ingestion API for large objects.
Team, Team task, and agent routes derive the owner scope from the authorized route, fetch the remote
HTTP(S) source on the server, stream bytes into the configured object store, verify final size and
SHA-256 when supplied, and publish `object_uploads` metadata only after the write verifies.

## Background

The 2026-07-22 decision changed the large-object path from browser multipart/presigned upload
planning to server-side download ingestion. The first implementation keeps browser JSON/base64
uploads as the small-object path and adds `/uploads/downloads` endpoints for external files.

## Scope

- Add route-derived download request handling for Team, Team task, and agent owner scopes.
- Add bounded object-store chunk writing so remote bytes are not materialized as one request body.
- Add first-pass SSRF guardrails: `http`/`https` only, no URL credentials, DNS validation, redirect
  revalidation, default private/local address rejection, byte limit, timeout, and checksum/size
  verification.
- Add operator config for max bytes, max redirects, timeout, and controlled private-network
  allowance.
- Add OpenAPI coverage for the new request schema and endpoints.

## Key Decisions

- Download endpoints are object-only in the first slice. Hosted images remain on `/images` so
  public rendering still goes through the existing raster MIME allowlist.
- The API is synchronous for now: a successful response returns the published `ObjectUploadRecord`.
  A queued intent table can be added later for cancel/expiry/error-class state without changing
  owner-scope authority.
- Metadata publication remains fail-closed. Stream errors abort the object writer; verification or
  metadata insert failures delete the written object before returning an error.

## Validation

```bash
cargo test -p agenthub-object-store -- --nocapture
cargo test -p agenthub download -- --nocapture
cargo test -p agenthub agent_upload_routes_publish_agent_scoped_metadata -- --nocapture
cargo test -p agenthub openapi -- --nocapture
cargo test -p agenthub-config object_store -- --nocapture
```

## Follow-Ups

- Add retry policy, per-host concurrency limits, and download observability before broad untrusted
  production exposure.
- Add an async intent table if product flows need queued/cancelable downloads or durable failed
  ingest state.

## 2026-07-28 Host Policy Hardening

### Summary

Server-side download ingestion now accepts operator-controlled source-host policy through
`[object_store].download_allowed_hosts` and `[object_store].download_denied_hosts`.

### Scope

- Added config parsing that trims, lowercases, de-duplicates, and ignores empty source-host policy
  entries.
- Added host validation before DNS/network access on the initial URL and every redirect hop.
- Kept the default behavior compatible: an empty allow list permits any otherwise-valid public
  HTTP(S) host, while deny entries always win.
- Supported exact host patterns and `*.example.com` wildcard subdomain patterns.

### Validation

```bash
cargo test -p agenthub-config object_store -- --nocapture
cargo test -p agenthub download_ -- --nocapture
```

### Follow-Ups

- Add retry policy, per-host concurrency limits, and download observability before broad untrusted
  production exposure.
- Add an async intent table if product flows need queued/cancelable downloads or durable failed
  ingest state.
