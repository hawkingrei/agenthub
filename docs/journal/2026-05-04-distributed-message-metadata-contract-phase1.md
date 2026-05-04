# Distributed Message Metadata Contract Phase 1

## Summary

This pass formalized AgentHub's distributed message metadata ownership model and landed the first
behavior change that aligns remote node replica ingestion with that contract.

## Background

Recent distributed-node and LanceDB work made it increasingly important to separate:

- canonical message authority on `main`;
- delivery metadata for cross-node relay;
- node-local cache or projection rows.

The repo already carried most of the required fields, but not one unified ownership contract.

## Scope

- add a canonical feature spec for logical message metadata ownership;
- add a canonical feature spec for distributed node registry and gossip boundaries;
- tighten one existing projection path so channel-replica ingestion explicitly requires
  `correlation_id` instead of treating it as an optional payload detail.

## Key Decisions

- `main` remains authoritative for canonical message truth and mailbox truth.
- non-`main` nodes keep only relevant message cache/projection rows.
- `authority_message_id` is the canonical logical message identity.
- `correlation_id` is required for channel-broadcast replica ingestion because remote node caches
  should keep both message identity and intent-lineage identity when indexing or replaying cached
  messages.

## Validation

Planned focused checks for this slice:

```bash
cargo test internal_grpc_mailbox_send_persists_channel_replica_history -- --nocapture
cargo test internal_grpc_mailbox_send_rejects_channel_replica_payload_without_correlation_id -- --nocapture
cargo fmt --all --check
```

## Follow-Ups

- add `group_id` to more physical schemas once multi-tenant/group rollout starts;
- continue reducing payload-only metadata conventions in favor of clearer persisted contracts;
- decide whether node-local archive/search stores need first-class cache tables or can continue to
  project directly from canonical payloads plus authority references.
