# 2026-03-19 Agent Node Review Hardening

## What Changed

- hardened gRPC relay delivery so route JSON can no longer provide `ca_cert_path`, `client_cert_path`, or `client_key_path`
- pinned gRPC relay delivery to registered `agent_nodes` by validating `route.grpc_target` and `route.tls_server_name` against the node registry before dialing
- moved relay TLS material sourcing to cluster-level internal gRPC defaults configured on `TeamManager`
- made remote-target agent creation fail fast when internal peer config is absent or when a legacy schema cannot persist `agents.target_node_id`
- installed the Rustls crypto provider in the shared internal gRPC mailbox client connect path
- restricted `agent_nodes` fetching and node-management UI to root sessions
- added client-side validation for inline node registration on the `Agents` page

## Validation

- `cargo test --locked create_agent_route_rejects_remote_target_without_internal_peer_client -- --nocapture`
- `cargo test --locked create_agent_rejects_remote_target_on_legacy_schema -- --nocapture`
- `cargo test --locked parse_remote_route_rejects_path_based_tls_fields_for_grpc -- --nocapture`
- `cargo test --locked grpc_relay_requires_registered_target_node_match -- --nocapture`
- `cargo test --locked remote_actor_grpc_pipeline_delivers_and_acks_over_tls -- --nocapture`
- `cargo test --locked bidirectional_actor_grpc_pipeline_relays_seeded_messages_between_in_process_states -- --nocapture`
- `cargo test --locked --test distributed_p2p_pipeline -- --nocapture`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `npm --prefix web run test -- src/app.route_auth.test.ts src/app.runtime_effects.test.tsx src/components/agent_node_section.test.tsx`

## Notes

- relay credentials are still route-scoped access tokens in phase 1; only TLS file material and destination routing are now centralized and pinned
- the root-only UI gating is a product boundary, not an authorization substitute; backend authorization remains required
