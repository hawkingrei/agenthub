# App Management API Checkpoint

## Summary

Authenticated App management now separates root connection provisioning, owner manifest publication,
Team approval, and explicit member binding. The runtime/shared-proxy integration remains open.

## Background

The [storage checkpoint](2026-09-17-app-registry-storage.md) established immutable versions and
durable grant epochs. Public management needs capability and ownership checks around those methods.

## Scope

Registry management and Team binding routes, safe response projections, bounded inputs, focused
HTTP authorization tests, and the shared proxy declaration-validation seam. The
[management contract](../features/app-management-api.md) lists routes, request shapes, pagination,
and errors. The proxy seam is not yet connected to production App launch mounts.

## Key Decisions

- Only `instance:configure` can choose endpoints and daemon environment credential references.
  Provisioning may assign another App owner; it does not grant a later ownership bypass.
- App-owner and Team-owner authority must intersect when approving scopes. Team owners subsequently
  select explicit member/version bindings within that grant and may revoke independently.
- A legacy unowned Team cannot use broad compatibility access to approve new external authority.
- Malformed JSON, private connection validation, and database failures use bounded payload-free
  errors. Serializable responses contain no connection fields.
- Registration and binding remain offline configuration; no endpoint calls or agent starts occur.
- Trusted integrations may pin discovery declarations before transport-specific filtering. A failed
  pinned catalog validation closes the session, so malformed or changed schemas cannot leave a stale
  catalog authorizing subsequent calls. Integrations without this optional validator are unchanged.

## Validation

Five HTTP workflow tests pass, covering capability/ownership denial, private field exclusion,
immutable publication, pagination, grant/binding revisions and scope intersection, revoked Apps,
and malformed/bounded requests. Three API capability-rule tests also pass.

The 37 shared proxy bridge tests pass, including two new pinned-manifest cases: unapproved tools
remain hidden, arguments retain their native shape and must satisfy the manifest, and input/output
schema drift closes an existing session. Invalid schemas and header annotations are included.
The final 37-test bridge rerun and all-target root/MCP Clippy pass with warnings denied. Formatting
and diff checks pass; the 18 local links in the four focused contract/journal documents resolve.

```bash
cargo +1.96.0 test -p agenthub --lib api::apps::tests --locked --offline
cargo +1.96.0 test -p agenthub --lib api::authz::tests --locked --offline
cargo +1.96.0 test -p agenthub-mcp --lib bridge::tests --locked --offline
cargo +1.96.0 clippy -p agenthub -p agenthub-mcp --all-targets --locked --offline -- -D warnings
cargo +1.96.0 fmt --all --check
```

## Follow-Ups

The [runtime checkpoint](2026-09-18-app-runtime.md) now supplies launch/preflight, authorization
against opened sessions, safe Cards, and pinned history. Complete final validation and CI delivery.
Signed events and the remaining transport slices are still separate unfinished parts of the goal.
