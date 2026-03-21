# 2026-03-18 Agent Node gRPC Control Plane

## What Changed

- Added `agent node` registry primitives to the `Agents` page and backend API.
- Added `target_node_id` to agent config/records so agents can bind to a remote node.
- Added internal gRPC remote-agent control methods for `ensure/start/stop/input/events`.
- Split remote relay transport logic into `src/team/manager/remote_relay.rs`.
- Added encrypted gRPC relay support for distributed actor delivery while keeping legacy HTTP relay compatibility.
- Added a simulated TLS/mTLS gRPC pipeline test that exercises:
  - source mailbox enqueue
  - source relay delivery
  - remote inbox fetch
  - remote ack
- Added a simulated TLS/mTLS gRPC pipeline test that exercises:
  - remote agent ensure
  - remote agent start
  - remote stdin input
  - remote event fetch
  - remote stop

## Backend Notes

- `agent_nodes` is a control-plane registry only.
- The remote relay gRPC route currently carries scoped connection material directly in the route payload for testability and incremental rollout.
- Destination mailbox records are normalized to local transport once the message arrives on the remote node.
- Remote agent shadow records stay in the main AgentHub DB, while the execution node receives a synced local record with `target_node_id = NULL`.
- Remote agent control peers authenticate with the shared internal gRPC auth/TLS configuration rather than per-node bootstrap secrets stored in the node registry.

## Frontend Notes

- `Create Agent` now lets the operator:
  - select `main` or a registered remote node
  - register/delete remote nodes inline
- Remote-node agents render a node badge in the left rail.
- Remote-node agents are startable from the current left rail through the same start action as local agents.
- Remote-node agents are excluded from local SSE fan-out; active workbench output continues through `/events` polling.

## Validation Plan

- `cargo test remote_agent_grpc_control_starts_inputs_and_lists_events_over_tls -- --nocapture`
- `cargo test remote_actor_grpc_pipeline_delivers_and_acks_over_tls -- --nocapture`
- `cargo test internal_grpc_mailbox_send_list_ack_are_wire_compatible -- --nocapture`
- `pnpm vitest --run web/src/agents_panel.test.tsx`
- `pnpm vitest --run web/src/sse_targets.test.ts`

## Remaining Gaps

- Node bootstrap / credential lifecycle is not yet surfaced in the `Agents` page.
- Remote list-card status is still shadow-state based; remote exit/restart events do not yet back-propagate into the main catalog automatically.
- Advanced ACP session mutation endpoints are not yet proxied for remote-node agents.
- Internal gRPC peer metadata is still reduced during relay delivery and should be expanded in a follow-up.
