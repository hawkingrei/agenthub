# Internal gRPC Fail-Fast Startup

## Summary

- Root cause: AgentHub started the internal gRPC server inside a detached task and logged
  `internal gRPC listening on ...` before `serve(addr)` had actually established the listener.
- Result: if bind/serve failed, the spawned task logged `transport error`, but the main web server
  still continued to boot. That left the process in a partially broken state where actor CLI
  mailbox/control commands timed out on internal gRPC connect.
- Fix: bind the internal gRPC listener synchronously before spawning the serving task, then log the
  bound address only after the listener exists.

## Implementation

- Added `bind_internal_grpc_incoming(...)` in `src/internal/mod.rs`.
- Switched startup from `serve(addr)` to `serve_with_incoming(incoming)`.
- Moved the `internal gRPC listening on ...` log after successful bind.
- Improved exit logging to include both display and debug error forms.

## Validation

- Focused regression test:
  - `cargo test -p agenthub maybe_spawn_internal_grpc_fails_fast_when_listen_addr_is_occupied -- --nocapture`
- Existing stalled-handshake timeout regression:
  - `cargo test -p agenthub connect_times_out_when_tls_peer_accepts_but_never_completes_handshake -- --nocapture`

## Follow-up

- After merge, verify one real startup on the deployed machine:
  - successful boot logs one `internal gRPC listening on ...` line and does not emit an immediate
    `internal gRPC server exited with error`
  - occupying the configured port makes AgentHub fail fast instead of continuing with the web
    server only
