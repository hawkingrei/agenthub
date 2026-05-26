# Team Remote Direct Mailbox Routing

## Summary

Added local regression coverage for Team task-message direct routing to a remote member and fixed the API mailbox-forward path so it preserves remote transport metadata instead of forcing every forwarded direct message onto `local/main`.

## Background

`docs/todo.md` tracks a `P1` follow-up for remote Team direct-mailbox routing on real multi-node teams. The existing mailbox remote tests already covered manager-level channel fanout, but the task-message API path still had a separate forwarding path. That path needed explicit verification because it builds mailbox envelopes after conversation writes rather than going through channel fanout orchestration.

## Scope

- Added an API regression test for inferred single-member direct routing when the recipient member is remote.
- Fixed task-message mailbox forwarding so recipient delivery resolution reuses the same remote-node lookup path as mailbox channel fanout.
- Kept the TODO open because this change does not replace real multi-node rollout validation.

## Key Decisions

- Reused `TeamManager` recipient-delivery resolution instead of duplicating `agents.target_node_id` lookup and grpc route construction inside `src/api/teams.rs`.
- Verified the API surface and stored mailbox row together:
  - conversation payload still normalizes `summary` and `detail_ref`
  - mailbox row switches to `transport=remote`
  - relay route carries `grpc_target`, `tls_server_name`, and `target_node_id`
  - mention metadata and reply target stay intact
- Left the real multi-node TODO item open with a narrower remaining tail instead of marking the whole rollout complete from a local regression test.

## Validation

```bash
cargo fmt --all --check
cargo test -p agenthub team_task_messages_api_routes_single_remote_mention_over_p2p_and_preserves_summary_metadata --target-dir /private/tmp/agenthub-remote-mailbox-target -- --nocapture
cargo check --target-dir /private/tmp/agenthub-remote-mailbox-target
```

## Follow-Ups

- Run the remote direct-mailbox path against a real multi-node Team deployment and record the relay evidence.
- Keep the existing manager-layer remote mailbox tests as the structural guardrail; use this API regression to catch future direct-route drift specifically.
