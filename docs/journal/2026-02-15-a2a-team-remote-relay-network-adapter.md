# A2A Team Remote Relay Network Adapter

## Summary

Replace the previous mock-only remote relay adapter with a real HTTP delivery
adapter, and add route-level auth/signing policy support for remote actor
messages.

## Background

`TeamRemoteRelayAdapter` previously used deterministic `mock://` endpoint
switches. This was enough for policy tests, but not sufficient for real remote
delivery. The Team TODO required moving to an actual network adapter with auth
and signing controls.

## Scope

- `src/team/manager/mailbox.rs`
- `src/team/manager/tests.rs`
- `docs/todo.md`

## Key Decisions

1. Remote relay route now supports real HTTP/HTTPS delivery:
   - `endpoint` (required),
   - `method` (optional, defaults to `POST`, allows `POST`/`PUT`/`PATCH`),
   - `headers` (optional key-value object),
   - `timeout_ms` (optional per-request timeout).
2. Auth policy is route-driven:
   - `auth.type = "bearer"` with `token`,
   - `auth.type = "header"` with explicit header `name/value`,
   - `auth.type = "basic"` with username/password.
3. Signing policy is route-driven:
   - `signing.type = "hmac_sha256"` with `secret`,
   - optional `header` and `timestamp_header` names.
4. Relay sends a structured JSON envelope including actor metadata and payload.
5. Retry/permanent classification:
   - `2xx` => delivered,
   - `429` and `5xx` => retryable,
   - other non-success => permanent (dead letter path).
6. Tests moved from `mock://` endpoints to local HTTP server assertions to
   verify real network behavior, headers, and envelope content.

## Validation

```bash
cargo test remote_actor_messages_relay_success_marks_message_delivered -- --nocapture
cargo test remote_actor_messages_relay_supports_retry_and_dead_letter -- --nocapture
```

## Follow-ups

- Add optional replay-protection fields (nonce / signature version) for stricter
  inter-service verification.
- Define a stable public schema for remote relay route config and document it in
  API docs.
