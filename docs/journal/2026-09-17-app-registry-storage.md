# App Registration Storage Checkpoint

## Summary

The storage foundation for slice 14 adds bounded App manifests, immutable versions, approved scopes,
explicit member bindings, and activation pinning. This is an implementation checkpoint; management
APIs, launch/proxy integration, Cards, and signed events are not yet delivered.

## Background

External tools need durable authority independent of provider processes. Running activations must
retain their selected manifest, while later permission changes must still deny future calls.

## Scope

This checkpoint covers the domain validator and additive database migration/store. Production launch
and proxy consumers are the next part of the same slice.

## Key Decisions

- Reuse the existing control database and loop executor validation. Do not create another scheduler
  or operation journal. The shared MCP proxy remains the integration target.
- Keep private connection/credential references out of serializable safe management records.
- Separate permission epochs from configuration revisions so version changes preserve running
  activations and revocation cannot be undone by later reapproval or rebinding.
- Freeze empty activation selections too. A startup retry retains the original app configuration;
  its replacement generation still needs a current lease and executor identity.
- Pin `jsonschema` 0.56.0 with default features disabled. Bound structure and serialized size before
  compilation, disable external retrieval, and use linear-time regular expressions.

The [storage contract](../features/app-registry-storage.md) records the concrete boundaries.

## Validation

The final combined regression passes 12 domain and 174 database tests. Its eight registry tests
cover startup retries, all three revocation boundaries, expired/foreign executors, and persisted
denial after reopen. All-target Clippy passes with warnings denied after replacing a redundant test
clone with a borrowed slice. The schema proof also rejects external HTTP/file references.

```bash
cargo +1.96.0 test -p agenthub-agent-domain app_tools --locked --offline
cargo +1.96.0 test -p agenthub-db app_registry --locked --offline
cargo +1.96.0 test -p agenthub-agent-domain -p agenthub-db --lib --locked --offline
cargo +1.96.0 clippy -p agenthub-agent-domain -p agenthub-db --all-targets --locked --offline -- -D warnings
```

## Follow-Ups

Finish the remaining slice 14 integration and end-to-end proxy checks before publishing the slice.
The active implementation goal still includes slices 15–18.
