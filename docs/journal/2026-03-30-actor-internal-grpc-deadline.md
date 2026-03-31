# Actor Internal gRPC Deadline Guard

## Summary

`agenthub actor inbox` and related Team mailbox/control commands could hang indefinitely when the internal loopback gRPC path stopped making progress. The immediate mitigation is to add explicit internal gRPC connect and request deadlines so stalled unary RPCs fail fast instead of leaving orphaned long-lived CLI subprocesses behind.

## Scope

- `src/internal/client/mod.rs`
- `src/internal/client/mailbox.rs`
- `src/internal/client/control.rs`
- `src/internal/client/tests.rs`

## Implementation

- Added a fixed internal gRPC connect timeout for actor-side mailbox/control clients.
- Added a fixed internal gRPC unary request timeout for the same clients.
- Normalized `DeadlineExceeded` into a user-visible timeout message so CLI failures explain that the internal control path stalled instead of appearing to hang forever.

## Validation

- `cargo test -p agenthub map_grpc_status_maps_common_codes -- --nocapture`
- `cargo test -p agenthub connect_times_out_when_tls_peer_accepts_but_never_completes_handshake -- --nocapture`
- `cargo test -p agenthub remote_actor_grpc_pipeline_delivers_and_acks_over_tls -- --nocapture`
- `./target/debug/agenthub actor inbox --actor-id 595d1ae8-fcbd-4111-b5c7-d446a12c044b --run-id shared-thread-mailbox:276a2682-9ce7-4af5-aa6c-f12575d13c37:7675f3e0-13ba-4b99-a58b-8ec39d6f3ff8 --limit 20`
- `cargo fmt --all`
- `cargo clippy --locked -p agenthub --all-targets -- -D warnings`
- `git diff --check`

## Follow-up

- Confirm the deployed server now returns timeout failures instead of hanging indefinitely for `agenthub actor inbox` / `ack` / `send`.
- Continue root-cause analysis on the underlying internal gRPC stall if it still reproduces after the deadline guard is in place.

## Verified Evidence

- Focused validation covered status mapping, stalled TLS handshake timeout, and a real mailbox
  command that now fails fast instead of hanging.
- `pull_request` CI for PR `#253`:
  - Bazel: `23723899703`
  - Rust: `23723899684`
  - Clippy: `23723899679`
  - Web: `23723899675`
  - Web E2E: `23723899706`
  - User Docs: `23723899695`
  - Distributed P2P Pipeline: `23723899677`
- default-branch `push` CI after merge commit `d9a9036c`:
  - Bazel: `23724117808`
  - Rust: `23724117810`
  - Clippy: `23724117803`
  - Web: `23724117802`
  - Web E2E: `23724117797`
  - User Docs: `23724117829`
  - Distributed P2P Pipeline: `23724117816`
