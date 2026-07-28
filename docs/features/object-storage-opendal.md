# Object Storage With OpenDAL

## Problem

AgentHub needs a storage boundary for uploaded files, generated artifacts, and future large
attachments that can outlive local browser sessions and move from local disk to AWS S3 or
S3-compatible services without rewriting every caller. Local paths alone are not a stable contract
for multi-node operation, cleanup, access checks, or remote upload flows.

## Scope

- A repository-owned object storage abstraction backed by Apache OpenDAL.
- A default local filesystem backend for development and single-node installs.
- An optional S3-compatible backend for AWS S3, Cloudflare R2, MinIO, and similar providers.
- Secret-free configuration shape for object store endpoints, buckets, prefixes, and credential
  environment variable references.
- Stable key, checksum, metadata, image-hosting, and publish/link rules that future file upload APIs
  must follow.

## Non-Goals

- Replacing SQLite metadata tables with object storage.
- Migrating existing Team context artifact rows in this slice.
- Exposing public object URLs as an authorization boundary.
- Implementing multipart uploads, resumable uploads, or presigned browser uploads in the first
  backend slice.
- Treating arbitrary remote URLs as safe object sources without SSRF, size, timeout, redirect, and
  checksum controls.
- Accepting SVG or HTML-backed image uploads in the first image-hosting slice.
- Storing access keys, secret keys, or session tokens in SQLite or committed config.

## Architecture

Object storage is split into two planes:

- Metadata/control plane: SQLite rows own resource identity, authorization scope, lifecycle state,
  checksum, size, MIME type, and attachment-to-message links.
- Data plane: OpenDAL writes and reads object bytes from the configured backend.

The first implementation adds `agenthub-object-store` as a focused crate. It owns key validation,
prefix joining, OpenDAL operator construction, byte writes, reads, deletes, existence checks, and
SHA-256/size reporting. The crate defaults to OpenDAL `fs`; S3 support is compiled behind the crate
`s3` feature so normal local and Bazel builds do not pull S3-only dependencies unless the release
profile asks for them.

Configuration lives under `[object_store]`:

```toml
[object_store]
backend = "fs"
root = "~/.agenthub/objects"
prefix = "agenthub/local"
download_allowed_hosts = ["downloads.example.com", "*.cdn.example.com"]
download_denied_hosts = ["metadata.google.internal", "169.254.169.254"]
download_retry_attempts = 3
download_retry_backoff_millis = 250
```

S3-compatible deployments use the same logical surface and keep credentials indirect:

```toml
[object_store]
backend = "s3"
bucket = "agenthub-artifacts"
endpoint = "https://s3.amazonaws.com"
region = "us-east-1"
prefix = "agenthub/prod"
public_base_url = "https://img.example.com"
access_key_id_env = "AGENTHUB_OBJECT_STORE_ACCESS_KEY_ID"
secret_access_key_env = "AGENTHUB_OBJECT_STORE_SECRET_ACCESS_KEY"
```

Agent-scoped CLI uploads and Team-scoped browser/API uploads use the same storage contract:

```bash
agenthub actor upload --file report.json --scope teams/team-1
agenthub actor upload --file screenshot.png --scope teams/team-1 --image
```

The actor CLI upload path owns command parsing and local file reading. Team upload API routes own
authorization, owner-scope derivation from the route, and request decoding. The shared
`ObjectUploadService` owns upload publication, metadata insertion, size/checksum verification, and
best-effort compensation. The database owns `object_uploads` metadata through a dedicated
`object_uploads` module. The object-store crate still owns byte storage only. If verification or
metadata publication fails after the byte write, AgentHub attempts to delete the just-written object
before returning the error. When `[object_store].root` is omitted for the local filesystem backend,
runtime state and CLI upload both default to `~/.agenthub/objects`.

The initial browser/API surface is intentionally Team-scoped and JSON/base64 based:

```text
POST /api/teams/{team_id}/uploads
POST /api/teams/{team_id}/images
```

Handlers derive `teams/<team_id>` from the authorized Team route and do not accept a raw owner scope
from the browser. The request includes `file_name`, `content_type`, `bytes_base64`, and optional
`expected_size_bytes` / `expected_sha256` verification fields. This inline surface is not the
canonical large-object path.

Large browser-originated objects should enter AgentHub through server-side download ingestion, not
browser multipart upload. The browser submits a download intent under the already-authorized Team,
task, or agent route:

```text
POST /api/teams/{team_id}/uploads/downloads
POST /api/teams/{team_id}/tasks/{task_id}/uploads/downloads
POST /api/agents/{agent_id}/uploads/downloads
```

The request names a source URL, file name, content type, optional expected size, and optional
SHA-256. AgentHub authorizes the route resource, derives the owner scope, downloads the source on
the server, streams bytes into the object store, verifies the final size/checksum, and publishes
SQLite metadata only after successful verification. The first implementation is synchronous at the
API boundary and returns the published `ObjectUploadRecord`; a future async intent table can add
queued/canceled/expired terminal states without changing the owner-scope authority.

Download ingestion must be fail-closed:

- allow only `http` and `https` source URLs;
- reject loopback, link-local, private, multicast, unspecified, and otherwise operator-blocked
  target addresses after DNS resolution and after each redirect;
- cap redirects, request timeout, retry attempts, retry backoff, and total bytes; optional
  per-host concurrency remains future
  operator hardening;
- never forward browser cookies, AgentHub credentials, authorization headers, or ambient provider
  secrets to the remote URL;
- require the final downloaded byte count to match `expected_size_bytes` when provided;
- require the final SHA-256 to match `expected_sha256` when provided;
- write to a server-generated object key before publishing metadata;
- abort the stream writer or delete the written object if verification or metadata publication
  fails.

S3 presigned and multipart routes remain backend transport tools for a future browser-upload
product path. They are not the default large-object ingestion path when the product action is
"download this external file into AgentHub."

Upload flows should follow a prepare/write/publish sequence:

1. Prepare a metadata intent with owner scope, expected size, MIME type, and optional checksum.
2. Generate a scoped object key from server-owned identifiers, never from raw user paths.
3. Write bytes through OpenDAL.
4. Verify actual size and checksum.
5. Publish or link the metadata row only after the durable write succeeds.
6. Clean up unlinked or failed intents from metadata state, not by blindly scanning object prefixes.

Image hosting is a specialized object-storage consumer. The storage layer provides a constrained
helper that writes raster image bytes under:

```text
images/<scope>/<image_id>.<extension>
```

Only `image/png`, `image/jpeg`, `image/webp`, and `image/gif` are accepted in the first slice. The
helper may return a `public_base_url`-derived URL when configured, but API handlers must still treat
that URL as a delivery address, not as the authorization decision.

The Team channel composer is the first graph-bed consumer. It accepts only the same raster image
types, reads the browser file locally, computes SHA-256 and byte length before upload, calls
`POST /api/teams/{team_id}/images`, then inserts a Markdown image link using the published upload
metadata. The inserted URL is for rendering/delivery only; future read, replace, delete, or presign
operations must still authorize against the Team-owned metadata row.

## Contracts

- Object keys are relative, slash-separated, and normalized before reaching OpenDAL.
- Empty keys, absolute paths, `.`/`..` segments, duplicate slashes, and backslashes are rejected.
- Callers may provide a logical key only inside their authorized resource scope; storage prefixes are
  joined by the object-store layer.
- SQLite metadata remains the authority for ownership, access control, lifecycle, and deletion
  eligibility.
- Object store presence does not grant read access. API handlers must authorize against the owning
  Team, channel, task, agent, or artifact row before reading bytes or issuing a future presigned URL.
- Browser large-object ingestion uses AgentHub-owned download intents. Source URLs are inputs to a
  server-side fetch job, not object-store authority and not metadata authority.
- Download requests bind caller, owner scope, source URL, file name, content type, expected size,
  expected SHA-256, and final object key. A future queued intent table must additionally bind
  expiry, status, and error class.
- Download completion must verify final size and SHA-256 before inserting a `published` metadata
  row. Failed, canceled, expired, or oversized downloads must not publish object metadata.
- Public image URLs must be issued only after the owner scope is validated and the metadata row is
  published. A CDN URL is not proof that a user may create, replace, or delete an image.
- Writes must record size and SHA-256 so repair and migration jobs can verify object integrity.
- Hosted images use server-generated object keys and allowlisted MIME types. SVG support needs a
  separate sanitization and content-security review before it can be enabled.
- Credentials are loaded from environment variables named by config, or by the provider chain when
  intentionally left unset; secret values must not be stored in DB rows, config fixtures, logs, or
  user-visible diagnostics.
- S3-compatible endpoints are treated as backend configuration, not product authority. AWS S3, R2,
  MinIO, and similar providers should share the same AgentHub metadata and key rules.

## Validation Matrix

| Area | Evidence |
| --- | --- |
| Key normalization | Unit tests reject path escape shapes and accept scoped relative keys. |
| Local backend | Focused async test writes, reads, checks existence, deletes, and verifies size/checksum. |
| Image hosting helper | Focused async test writes a scoped raster image object, rejects nested image ids, and returns a normalized public URL. |
| Agent upload entry | Parser, owner-scope, and DB tests cover `agenthub actor upload`, required scope/file flags, image mode, and published metadata persistence. |
| Team upload API | Handler and router tests cover authorization, owner-scope derivation, base64 upload publication, raster-image allowlist, and size/checksum mismatch rejection without publishing metadata. |
| Task and agent upload APIs | Focused API tests cover parent Team authorization before task-scope publication, agent existence checks before agent-scope publication, object/image key prefixes, and OpenAPI fixture coverage for every new route. |
| Large download ingestion | Service and API tests cover route-derived owner scopes, URL scheme rejection, private/loopback address rejection, operator source-host allow/deny policy, configurable private-network allowance for controlled tests, final size/SHA-256 verification, OpenAPI coverage, and successful metadata publication after a streamed server-side download. Object-store tests cover chunked writes with size/hash calculation. |
| Graph-bed UX | Frontend tests cover raster MIME allowlisting, browser SHA-256/base64 request preparation, Team image endpoint wiring, and Markdown image insertion in the Team channel composer. |
| Config contract | `agenthub-config` tests confirm defaults and secret-free S3 env reference trimming. |
| Bazel coverage | `//crates/agenthub-object-store:agenthub_object_store_tests` is listed in Bazel test and coverage targets. |
| S3-compatible fixture | A MinIO-backed CI job runs `agenthub-object-store` with the `s3` feature and verifies write/read/exists/delete plus hosted-image URL behavior against a real S3-compatible endpoint. |
| Future S3 rollout | Keep S3 out of release feature sets until the MinIO fixture is green in PR and push CI and one reviewed release build includes the feature intentionally. |

## Operational Notes

- The default local root lives under `~/.agenthub/objects` for shared runtime and actor CLI uploads
  when `backend = "fs"` and no explicit root is configured.
- Prefixes should include deployment or tenant scope when several AgentHub instances share one
  bucket.
- `public_base_url` is optional. Use it only for deployments with a reviewed CDN/object gateway path
  and keep mutating API authorization on AgentHub metadata.
- Cleanup should be metadata-driven: delete unlinked failed uploads only after their metadata state
  is older than the grace period and no published row references the same object key.
- Download ingestion should stream through a bounded buffer into the object store and compute
  SHA-256 while streaming. It must not materialize the full remote object in memory.
- Operators can configure maximum download bytes, redirect limits, timeout limits, retry attempts,
  retry backoff, whether private networks are allowed for controlled deployments/tests, and
  source-host allow/deny lists. Host policy defaults to allow all non-private HTTP(S) sources,
  denies exact or wildcard-denied hosts first, and requires an exact or wildcard allow match when
  `download_allowed_hosts` is non-empty. Download ingestion retries only pre-stream transient source
  request failures: request timeout, 429, 5xx, connection errors, and client timeouts.
- Download ingestion emits structured logs for retry attempts and terminal success/failure with
  source host, owner scope, upload id, latency, byte count on success, and failure class. S3/R2/MinIO
  production use should add durable counters before large user-facing uploads become default-on.
- Existing local Team context artifacts remain compatible until their metadata schema is explicitly
  migrated to record backend/key instead of only absolute local paths.

## Open Risks

- Existing artifact tables store local filesystem paths; moving those rows to object storage needs a
  schema migration and read-compatibility plan.
- Download ingestion has first-pass SSRF guardrails, operator source-host policy, bounded pre-stream
  retry, structured logs, and streaming object-store writer support, but still needs per-host
  concurrency limits and durable metrics counters before broad untrusted production exposure.
- S3-compatible providers differ in multipart, path-style, and checksum behavior; MinIO is the first
  CI fixture, but each documented production provider still needs compatibility evidence before it
  is described as production-ready.
- Object lifecycle bugs can leak storage cost or delete still-referenced bytes unless metadata
  reference checks are kept explicit.

## Source Journals

- [2026-07-16-object-storage-opendal.md](../journal/2026-07-16-object-storage-opendal.md)
- [2026-07-22-object-storage-download-ingest.md](../journal/2026-07-22-object-storage-download-ingest.md)
