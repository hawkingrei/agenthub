# Agent Nodes

## Problem

AgentHub needs a node-level abstraction for distributed execution and actor delivery without collapsing all runtime data into the main process. The control plane must know which node an agent targets, the UI must let operators manage nodes directly from the `Agents` page, and cross-node actor transport must move to encrypted gRPC.

## Scope

- add `target_node_id` to agent config and persisted agent records
- add `agent_nodes` registry CRUD for the standalone `Agents` flow
- proxy remote-node agent lifecycle control over encrypted internal gRPC
- add encrypted gRPC transport support for distributed actor relay
- add a simulated gRPC pipeline test that covers relay delivery into a remote mailbox and ack
- add a simulated in-process bidirectional gRPC relay regression test that covers node-to-node relay ordering and ack
- add a dedicated blackbox distributed p2p pipeline that boots two real AgentHub nodes and validates relay worker delivery plus inbox ack over gRPC
- add a simulated TLS/mTLS gRPC pipeline test that covers remote agent ensure/start/input/events/stop
- keep node-local persistence limited to that node's local agent/runtime data

## Non-Goals

- production-grade workload identity / certificate rotation
- remote ACP session mutation parity for every advanced workbench action
- complete peer metadata preservation across every internal gRPC mailbox hop

## Architecture

### Node Registry

- `agent_nodes` stores:
  - `id`
  - `name`
  - `grpc_target`
  - `tls_server_name`
  - `default_worktree_root` (optional)
- `main` is a reserved built-in node identifier and always resolves to the local process.
- `AgentConfig` / `AgentRecord` now carry `target_node_id`.
- Root operators can update registered node routing and `default_worktree_root` in place without re-registering the node.

### Actor Transport

- Team mailbox relay accepts a gRPC route payload:

```json
{
  "kind": "grpc",
  "grpc_target": "https://node-east.internal:50051",
  "access_token": "<scoped-token>",
  "tls_server_name": "node-east.internal"
}
```

- Relay delivery uses `TeamInternalControl/SendActorMessage` over TLS/mTLS.
- After a remote relay succeeds, the destination node stores the received mailbox message as local transport so the target actor can consume it without another relay hop.
- Legacy HTTP relay routes remain accepted for compatibility while node transport moves to gRPC.
- gRPC relay routes must target a registered `agent_node`; route-level `grpc_target` and `tls_server_name` are validated against the registry entry before a connection is opened.
- Route-level TLS file paths are rejected; TLS client material is derived from cluster-level internal gRPC configuration, not from user-provided relay JSON.

### Remote Agent Control

- AgentHub uses the same internal gRPC control plane to:
  - ensure a remote node has the target agent record
  - start a remote agent with optional actor runtime context
  - proxy stdin input into the remote runtime
  - read remote event history for the active workbench
  - stop a remote runtime
- Before syncing to the execution node, the control plane strips `target_node_id` so the node-local database only stores local agent records.
- Peer auth/TLS material is derived from cluster-level internal gRPC config, while the node registry stays focused on routing (`grpc_target`, `tls_server_name`).
- When a remote-target agent uses `create_worktree` and leaves `workdir` blank, AgentHub now derives the runtime root from the selected node's `default_worktree_root`. If the node has no default root configured, remote create-worktree requests must still provide an explicit `workdir`.

### Frontend

- `Agents -> Create Agent` includes:
  - execution-node selection
  - root-only remote node registration fields
  - root-only inline node config editing
  - remote node deletion controls
- Agent rows show `node:<id>` when bound to a remote node.
- Remote-node agents can now be started from the `Agents` page through the same start action as local agents.
- Remote-node agents are excluded from local SSE fan-out because their live output is fetched through `/events` polling against the remote node.
- Non-root sessions do not fetch `agent_nodes` and do not render inline node-management controls, so the page no longer produces repeated `401` noise for regular users.
- The create-agent modal switches its create-worktree placeholder to the selected node's default root so operators can see which remote runtime root will be used before submitting.

## Contracts

- node-to-node and node-to-AgentHub actor delivery must use `https://` gRPC targets
- node-local DB backup must only store that node's local agent/runtime data
- cluster/team/control-plane metadata remains authoritative in the main AgentHub database

## Validation Matrix

Recommended commands:

```bash
cargo test remote_agent_grpc_control_starts_inputs_and_lists_events_over_tls -- --nocapture
cargo test remote_actor_grpc_pipeline_delivers_and_acks_over_tls -- --nocapture
cargo test bidirectional_actor_grpc_pipeline_relays_seeded_messages_between_in_process_states -- --nocapture
cargo test --test distributed_p2p_pipeline -- --nocapture
cargo test internal_grpc_mailbox_send_list_ack_are_wire_compatible -- --nocapture
pnpm vitest --run web/src/agents_panel.test.tsx
pnpm vitest --run web/src/sse_targets.test.ts
```

GitHub Actions:

- `Rust (Cargo)` runs the distributed gRPC integration step explicitly before workspace coverage:
  - `remote_actor_grpc_pipeline_delivers_and_acks_over_tls`
  - `bidirectional_actor_grpc_pipeline_relays_seeded_messages_between_in_process_states`
  - `remote_agent_grpc_control_starts_inputs_and_lists_events_over_tls`
- `Distributed P2P Pipeline` is the standalone blackbox workflow that boots two real AgentHub nodes and validates bidirectional mailbox relay plus ack over background workers.

Manual checks:

1. Open `Agents -> Create Agent`.
2. Register a remote node with an `https://` gRPC target.
3. Update that node and set `default_worktree_root`.
4. Select that node for a new `create_worktree` agent, leave `Workdir` blank, start it directly from the `Agents` page, and confirm the card shows `node:<id>`.
5. Open the agent workbench and confirm remote output appears through event polling after start/input.
6. Confirm local `main` execution remains the default selection.
7. Log in as a non-root user and confirm the `Agents` page does not attempt `agent_nodes` admin requests.

## Operational Notes

- gRPC relay route material currently carries scoped credentials directly so the relay pipeline can be exercised before node bootstrap is fully surfaced in the UI.
- Remote agent control currently depends on cluster peers sharing internal gRPC auth/TLS configuration (shared secret / CA trust chain); the node registry itself does not store bootstrap secrets.
- Remote node shadow records remain in the main AgentHub DB, while the execution node persists only its local runtime record with `target_node_id = NULL`.
- Remote-target agent creation now fails fast if the cluster has no internal peer config or if the local schema cannot persist `agents.target_node_id`; AgentHub does not create shadow records that it cannot later control.
- The phased scale-out architecture, including gossip boundaries and the shared-key to zero-trust migration path, is documented in `docs/features/distributed-node-architecture.md`.

## Open Risks

- Team runtime should derive node-scoped actor credentials from node registry/bootstrap instead of route-level gRPC credentials.
- Remote status reconciliation is still shadow-state based for list views; remote exits are not yet streamed back into the main AgentHub catalog.
- Advanced ACP session mutation endpoints (`clear session`, `set mode/model/config`, `cancel`) are not yet proxied over the remote node control plane.
- Peer metadata propagation over internal gRPC is still reduced during relay delivery.

## Source Journals

- `docs/journal/2026-03-18-agent-node-grpc-control-plane.md`
- `docs/journal/2026-03-19-agent-node-review-hardening.md`
- `docs/journal/2026-03-19-agent-node-config-and-userdocs.md`
