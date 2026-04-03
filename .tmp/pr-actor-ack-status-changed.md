## Summary

- surface `status_changed` in `agenthub actor ack` responses
- plumb the field through Team mailbox storage, internal gRPC, and the actor CLI
- document the new ack diagnostic semantics and add focused regression coverage

## Validation

- `cargo test -p agenthub actor_ack_reports_noop_when_message_is_already_delivered -- --nocapture`
- `cargo test -p agenthub ack_actor_messages_batches_requests_in_order -- --nocapture`
- `cargo test -p agenthub internal_grpc_mailbox_send_list_ack_are_wire_compatible -- --nocapture`
- `cargo test -p agenthub internal_grpc_uses_remote_mailbox_client_when_runtime_env_is_present -- --nocapture`
- `git diff --check`
