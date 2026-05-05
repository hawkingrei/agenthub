# Message Metadata Surface Rollout Phase A

## Summary

This pass extends the archive/search projection contract so archive documents can preserve the same
message-identity references already being tightened on Team relay and replica paths.

## Scope

- promote `authority_message_id` and `correlation_id` into the archive document model as first-class
  optional fields;
- keep `logical_message_id` for generic/non-Team logical grouping, especially aggregated ACP
  messages;
- make LanceDB schema plus search filtering understand the new fields;
- avoid broader ingest wiring changes in this slice.

## Key Decisions

- `authority_message_id` is the Team-facing canonical message identity for archive/search
  projections when the source record ultimately reconciles to a canonical conversation message.
- `correlation_id` should stay first-class in archive documents so search/replay can preserve
  intent lineage instead of re-parsing payload JSON everywhere.
- `logical_message_id` is still needed, but its meaning is narrower: it remains the generic logical
  grouping field for sources such as aggregated ACP messages where there is no Team authority
  message row.

## Validation

Validated with:

```bash
cargo test -p agenthub-message-archive -- --nocapture
cargo check -p agenthub-message-archive
```

## Follow-Ups

- wire the archive document contract into the first real dual-write ingress paths instead of
  keeping it test-only inside the archive crate;
- continue the remaining message-surface matrix so relay payloads, node-local caches, and archive
  docs all carry the same canonical identity references where applicable;
- plan the later `group_id` rollout separately from this archive-document identity pass.
