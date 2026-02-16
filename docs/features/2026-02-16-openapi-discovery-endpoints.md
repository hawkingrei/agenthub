# OpenAPI Discovery Endpoints

## Background

Team APIs have grown quickly, but there was no built-in endpoint to inspect a machine-readable
contract in one place. This made frontend and integration debugging slower.

## Scope

- Add `GET /api/openapi.json` for authenticated OpenAPI discovery.
- Add `GET /api/openapi/docs` for a lightweight browser page that fetches and renders
  `/api/openapi.json`.
- Cover current Team Workbench related contracts in the generated OpenAPI document, including
  team runs listing (`GET /api/teams/{id}/runs`).
- Add tests for:
  - unauthorized access to `/openapi.json`
  - spec presence for key team paths

## Key Decisions

- Keep `/api/openapi.json` authenticated to match existing API access control semantics.
- Keep `/api/openapi/docs` public as a static viewer page, while the actual spec fetch still uses
  Bearer auth from `localStorage.agenthub_auth`.
- Start with Team-focused path coverage and extend incrementally as new API domains stabilize.

## Validation

Suggested checks:

```bash
cargo test openapi_json_requires_authorization -- --nocapture
cargo test openapi_json_contains_team_runs_list_path -- --nocapture
cargo test teams_router_http_contract -- --nocapture
```

Manual checks:

1. Open `/api/openapi/docs` in a logged-in browser session.
2. Verify the page can load `/api/openapi.json` and render formatted JSON.
3. Verify clearing `agenthub_auth` causes `/api/openapi.json` fetch to return `401`.
