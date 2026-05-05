# Message Archive Group ID Projection

## Summary

This pass adds `group_id` as a nullable archive/search projection field without assigning a live
default group to current Team writes.

## Background

The logical message metadata contract identifies `group_id` as the future isolation key for
multi-tenant and multi-group routing. Previous archive work promoted `authority_message_id` and
`correlation_id` into first-class projection fields, but `group_id` still only existed as a design
target.

## Scope

- Add `group_id` to the backend-agnostic archive document, search query, and search hit models.
- Add LanceDB schema storage, legacy-table migration, search projection, and filtering support for
  `group_id`.
- Keep current Team conversation, run-event, actor-message, and ACP archive writes with a null
  `group_id` until a live authority-layer group value exists.
- Surface `group_id` in Team archive search responses when the archive backend returns it.

## Key Decisions

- `group_id` is a projection compatibility field in this slice, not a new authority value.
- Existing Team writes must not invent a default group because that would make later multi-tenant
  semantics harder to distinguish from true authority data.
- Public Team-scoped message search still forces `team_id` from the route. `group_id` is returned
  as hit metadata, but not accepted as a caller-controlled Team search scope in this pass.

## Validation

Planned focused checks:

```bash
cargo test -p agenthub-message-archive -- --nocapture
cargo test -p agenthub team_message_search_api_uses_archive_with_team_scope -- --nocapture
cargo check -p agenthub-message-archive
```

## Follow-Ups

- Populate `group_id` from real authority rows once the live multi-tenant/group schema lands.
- Extend node-local caches and relay projections to preserve `group_id` after those authority
  surfaces carry it.
