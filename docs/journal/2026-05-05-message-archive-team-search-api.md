# Message Archive Team Search API

## Summary

Team message search now has a backend route that reads through the message archive abstraction
instead of adding another SQLite-specific message scan.

## Background

The message archive rollout already introduced the backend-agnostic archive trait, LanceDB as the
first backend, first Team conversation dual-write, and archive document metadata for
authority-linked projections. The next useful search slice is a small Team-scoped API boundary that
can feed later UI search without exposing LanceDB details to Team API callers.

## Scope

- Add `GET /api/teams/:id/messages/search`.
- Enforce normal Team ownership checks before any archive query runs.
- Force archive `team_id` scope from the route path.
- Return archive hit metadata needed for future deep links.
- Keep the search route backend-only in this slice; no frontend search UI changes are included.

## Key Decisions

- The Team API calls `TeamManager::search_message_archive`, and `TeamManager` depends only on the
  archive trait object. API code does not depend on LanceDB connection or query-builder types.
- Caller-provided Team scope is intentionally ignored. The path Team id is the only Team filter the
  handler forwards to the archive.
- Search result metadata is part of the public response now so future UI slices do not need to
  parse `payload_json` to recover `authority_message_id`, `correlation_id`, task, run, agent, or
  session context.
- A missing archive backend returns an empty result set for this first API slice, matching the
  best-effort rollout posture used by live dual-write initialization.

## Validation

```bash
cargo fmt --all --check
cargo test --lib team_message_search_api_uses_archive_with_team_scope -- --nocapture
cargo test -p agenthub-message-archive lancedb_archive_can_append_and_search_messages -- --nocapture
cargo check
```

## Follow-Ups

- Wire the frontend workspace search surface to the Team-scoped archive search API.
- Add historical migration coverage before treating archive search as complete for old SQLite-only
  rows.
- Extend read/search routes for additional message surfaces after their dual-write and migration
  paths land.
