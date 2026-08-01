# Object Upload Large Object Path

## Summary

This checkpoint closes the large-object upload path decision for browser-facing object storage and
adds the first backend foundation for that path. Small Team/task/agent uploads remain on the
existing JSON/base64 API, while large browser uploads use AgentHub-issued upload sessions that may
either proxy bytes through AgentHub or expose a short-lived S3 whole-object presigned write without
making object storage the authorization authority.

## Background

The owner-scope rollout established that API routes derive `teams/<id>`, `tasks/<id>`, and
`agents/<id>` scopes from authorized resources. Browser callers cannot provide raw owner scopes.
The remaining design question was whether large uploads should become generic multipart POSTs
through AgentHub or direct presigned uploads to the configured backend.

## Scope

- Decide the canonical large-object browser path.
- Keep the current JSON/base64 upload surface as the small inline path.
- Add an explicit inline upload size guard.
- Add persisted upload-session metadata plus prepare/cancel service operations.
- Add Team/task/agent HTTP prepare/cancel/complete routes that derive owner scope from authorized
  resources.
- Add the first AgentHub proxy complete path: the complete route accepts `application/octet-stream`,
  writes the prepared object key, verifies size/checksum, publishes `object_uploads` metadata, and
  marks the session completed.
- Add expiry cleanup for stale prepared sessions.
- Add provider-neutral proxy part uploads plus complete-from-parts routes for Team, task, and agent
  upload sessions.
- Use bounded-memory object-store writer completion for proxy parts.
- Add S3 whole-object presigned direct-write issuance plus direct completion that streams the stored
  prepared-key object for size/checksum verification before metadata publication.
- Add low-level S3 multipart object-store primitives for initiate, upload-part presign, complete,
  and abort.
- Wire the S3 multipart primitives into Team, task, and agent upload-session service/API routes for
  initiate, presigned part issuance, complete, and abort.

## Key Decisions

- Large browser uploads should use short-lived AgentHub upload sessions.
- The session is scoped to the authorized owner resource, expected size, content type, checksum, and
  object kind before any storage write starts.
- S3-compatible backends may return a whole-object presigned `PUT` for the prepared session key.
  Completion still returns to AgentHub so SQLite metadata can verify checksum/size and publish the
  upload row.
- OpenDAL 0.58 exposes public whole-object `presign_write`, but multipart part presign/complete
  remains backend-internal. AgentHub should not depend on those private internals.
- AgentHub owns a small path-style S3 REST shim for multipart direct uploads when explicit S3
  endpoint, bucket, region, access key, and secret key settings are present. OpenDAL remains the
  default byte-storage abstraction for normal read/write/delete behavior.
- Filesystem and non-presign-capable backends may use the same session state with an AgentHub
  chunk-proxy writer rather than exposing provider-specific details to the browser.
- Public URLs remain delivery addresses only; they are not create, replace, read, delete, or publish
  authority.
- `object_upload_sessions` is the metadata anchor for future large-object writes. Prepared sessions
  bind owner scope, backend, planned object key, object kind, expected size/checksum, creator,
  status, and expiry before any bytes are written.
- Team, task, and agent prepare/cancel routes do not accept raw owner scopes. They reuse the same
  route-derived authority as the inline JSON/base64 upload routes.
- Complete routes publish metadata only after the object bytes are written to the prepared key and
  the stored size/checksum match the session expectation.
- Publish-time DB transactions require the `object_uploads.owner_scope` value to match the prepared
  session owner scope before marking the session completed, so callers cannot publish metadata under
  a different owner by bypassing the service-level scope guard.
- Expiry cleanup is a DB-owned terminal state transition. It marks only `prepared` sessions whose
  `expires_at` is older than the cleanup cutoff as `expired`, leaving completed, canceled, and
  still-valid prepared sessions untouched.
- Proxy part uploads use session-scoped temporary object keys plus an ordered SQLite manifest.
  Completion requires contiguous parts starting at `1`, verifies the recorded part bytes, streams
  each part into the prepared final object key through the object-store writer, checks final
  size/checksum against the prepared session, publishes metadata, then removes temporary part
  objects after successful publication.
- Direct-write completion does not trust object-store visibility alone. It streams the existing
  prepared-key object by range, recomputes SHA-256 and size, verifies them against the prepared
  session, and only then publishes metadata.
- Direct multipart completion follows the same metadata authority boundary: the browser may upload
  parts with short-lived S3 presigned URLs, but AgentHub still owns upload-session scope, size and
  checksum verification, metadata publication, and abort/cancel semantics.

## Validation

```bash
cargo test decode_upload_bytes_rejects_payloads_above_inline_limit
cargo test -p agenthub-db object_upload_session_lifecycle_persists_prepared_and_canceled_states
cargo test -p agenthub-db init_db_creates_schema_and_enforces_foreign_keys
cargo test prepare_upload_session
cargo test team_upload_session_api_prepares_and_cancels_team_scope
cargo test team_task_upload_session_api_derives_task_scope
cargo test agent_upload_routes_publish_agent_scoped_metadata
cargo test openapi_json_contains_team_runs_list_path
cargo test complete_upload_session
cargo test -p agenthub-db publish_object_upload_session_inserts_upload_and_marks_session_completed
cargo test -p agenthub-db publish_object_upload_session_rejects_owner_scope_mismatch
cargo test -p agenthub-db cleanup_expired_object_upload_sessions_marks_only_expired_prepared_sessions
cargo test cleanup_expired_upload_sessions_marks_prepared_sessions_expired
cargo test -p agenthub-db object_upload_session_parts_upsert_and_list_in_order
cargo test upload_session_parts_complete_resumable_proxy_upload
cargo test -p agenthub-object-store stored_key_writer_writes_chunks_and_reports_digest
cargo test -p agenthub-object-store put_stored_key_bytes_writes_planned_prefixed_key_without_double_prefixing
cargo test -p agenthub-object-store fs_store_rejects_presigned_stored_key_writes
cargo test -p agenthub-object-store fs_store_rejects_s3_multipart_uploads
cargo test -p agenthub-object-store inspect_stored_key_streams_size_and_digest
cargo test complete_direct_upload_session_verifies_planned_key_and_publishes_metadata
cargo test upload_session
cargo test -p agenthub-object-store --features s3 --locked s3_compatible_store_exercises_bytes_and_hosted_images -- --nocapture
cargo test --features object-store-s3 --locked team_upload_session_s3_multipart_route_fixture_publishes_metadata -- --nocapture
```

## Follow-Ups

- Keep multipart browser uploads behind the S3 release gate until the MinIO-backed object-store and
  API route fixtures are green in PR and push CI.
