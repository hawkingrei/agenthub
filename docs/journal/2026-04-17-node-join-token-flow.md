# Node Join Token Flow

## Summary

- added a root-only `GET /api/agent_nodes/bootstrap` surface so the web app can show current node bootstrap details without reusing the device-join QR flow
- updated the `Agents` page to present a token-first `Join node with token` helper before manual node registration
- removed Admin QR-specific join presentation in favor of token plus join link copy
- refreshed user docs and feature docs so Agent Node onboarding now points at `internal_grpc.bootstrap.token`

## Validation

- Rust:
  - `cargo test get_agent_node_bootstrap_returns_root_only_join_info -- --nocapture`
- Web:
  - `cd web && npm run test -- vite.config.test.ts src/components/agent_node_section.test.tsx src/use_app_agents.test.tsx src/pages/admin_page.test.tsx src/components/agents_route_modal_props.test.ts`

## Notes

- Agent Node bootstrap is token-based; QR onboarding remains a device/browser join concern only.
- The node registry still stores routing only. Shared internal gRPC TLS/auth configuration is still cluster-wide.
- Follow-up hardening on the PR branch added an explicit `Agent Node Join Bootstrap` error state in the Agents UI, URL-encoded join-link tokens, and a copy-link affordance for Admin device join.
- Chrome DevTools MCP baseline/regression check could not be completed in this environment because the transport closed before page enumeration.
