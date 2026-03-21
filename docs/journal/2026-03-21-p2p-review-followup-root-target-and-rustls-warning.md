# P2P Review Follow-Up: Root Target Guard And Rustls Warning

## Summary

- Enforced the root-only execution-node contract in `POST /api/agents`.
- If a non-root caller supplies a non-local `target_node_id`, the API now rejects the request before agent creation.
- `internal::tls::install_rustls_crypto_provider()` now emits a warning when provider installation fails instead of silently discarding the error.

## Validation

- `cargo test create_agent_route_rejects_remote_target_for_non_root_user -- --nocapture`
- `cargo test create_agent_route_rejects_remote_target_without_internal_peer_client -- --nocapture`
- `git -c core.fsmonitor=false diff --check`
