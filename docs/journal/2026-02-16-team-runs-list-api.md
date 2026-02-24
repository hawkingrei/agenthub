# Team Runs List API

## Background

Team APIs already supported team create/run create/run details, but there was no endpoint to list runs
for a specific team. This made UI reconstruction after refresh/reconnect difficult and forced
client-side run state caching.

## Scope

- Add `GET /api/teams/:id/runs`.
- Add query options:
  - `limit` (default 100, clamp `1..500`)
  - `status` (one of `submitted|working|input_required|completed|failed|canceled`)
  - `before_created_at` (cursor for older pages)
- Add manager-layer method to query team runs with ordering and optional filters.
- Add API tests (core + router) for:
  - happy path listing
  - status filter
  - cursor paging by `before_created_at`
  - invalid status validation
  - missing team behavior (`404`)

## Key Decisions

- Keep cursor semantics simple and stable with `before_created_at` for now.
- Order by `created_at DESC, id DESC` to provide deterministic paging order for identical timestamps.
- Reuse existing auth and not-found mapping semantics (`require_user`, `map_not_found_error`).

## Validation

Suggested checks:

```bash
cargo test team_runs_api_lists_team_runs_with_status_filter_and_cursor -- --nocapture
cargo test teams_router_http_contract -- --nocapture
cargo test -p agenthub-web
```

## Validation Evidence (2026-02-20)

- Command:
  - `cargo test team_runs_api_paginates_high_volume_without_duplicates_and_honors_status_filter -- --nocapture`
- Result:
  - passed `api::teams::tests::team_runs_api_paginates_high_volume_without_duplicates_and_honors_status_filter`:
    - seeds 120 runs under one team, pins deterministic `created_at`, and verifies full cursor pagination (`before_created_at`) without duplicates.
    - verifies global ordering remains `created_at DESC, id DESC` across pages.
    - verifies `status=canceled` filter remains correct across multi-page retrieval.
