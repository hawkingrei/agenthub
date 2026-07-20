# Message Archive Lance 8 Upgrade

## Summary

The message archive LanceDB backend now tracks the compatible Lance 8 dependency family by upgrading `lancedb` to `0.31.0` and `lance-index` to `8.0.0`.

## Background

The standalone `lance-index` and `arrow-array` Dependabot updates were not independently mergeable. `lancedb` owns the Arrow and Lance types exchanged by the archive backend, so direct dependency versions must stay aligned with the `lancedb` public API instead of mixing adjacent version families.

## Scope

- Upgrade `lancedb` from `0.30.0` to `0.31.0`.
- Upgrade `lance-index` from `7.0.0` to `8.0.0`.
- Keep direct `arrow-array` and `arrow-schema` dependencies on `58.3.0` because `lancedb 0.31.0` still exposes Arrow 58 types.

## Key Decisions

- Do not merge the Arrow 59 direct dependency bump yet. It creates distinct `RecordBatch` and `Schema` types from the Arrow 58 values returned by `lancedb`.
- Treat the Lance index upgrade as a coordinated dependency-family bump, not as an isolated package update.

## Validation

```bash
cargo check -p agenthub-message-archive
cargo test -p agenthub-message-archive
```

Both commands passed locally against the coordinated Lance 8 / Arrow 58 dependency set.

## Follow-Ups

- Revisit the direct Arrow 59 bump only after a `lancedb` release exposes Arrow 59-compatible public types.
