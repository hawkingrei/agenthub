# P2P Coverage Follow-Ups

## Summary

Added focused regression tests for remote-target normalization and internal gRPC bootstrap validation to improve coverage on the P2P control-plane branch without expanding the feature scope.

## Changes

- Added internal service coverage in `src/internal/service.rs`.
  - Reject mismatched bootstrap tokens during `issue_node_credential`.
  - Reject worker bootstrap requests missing `actor_id` or `run_id`.
- Added remote-target normalization coverage in `src/api/agents.rs`.
  - `target_node_id = "main"` now has direct regression coverage proving it normalizes back to local agent storage.
- Normalized persisted `target_node_id` values when reading agent records in `src/agent/manager.rs`.
  - Local-equivalent values such as `main` are collapsed back to `None` on read so API/UI state stays consistent with runtime routing.
- Added agent-node reserved-id API coverage in `src/api/agent_nodes.rs`.
  - Deleting the reserved `main` node now has explicit route coverage for the bad-request path.

## Validation

- `cargo test issue_node_credential_rejects_bootstrap_token_mismatch -- --nocapture`
- `cargo test issue_node_credential_requires_worker_actor_and_run -- --nocapture`
- `cargo test create_agent_treats_main_target_node_as_local -- --nocapture`
- `cargo test delete_main_agent_node_returns_bad_request -- --nocapture`
