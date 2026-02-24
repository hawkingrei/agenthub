# Team Delete API and UI

## Background

Team workbench users can create and run many team executions, but there was no way to remove obsolete teams from either API or UI. This caused stale entries to accumulate and made local single-node usage harder to maintain.

## Scope

- Add `DELETE /api/teams/:id` for removing a team definition and all team-owned runtime data.
- Add Team Workbench UI action (`Delete Team`) for deleting the currently selected team.
- Keep OpenAPI docs aligned with the new endpoint.
- Add backend and frontend tests that cover deletion behavior.

## Key Decisions

1. Use explicit transactional cascade in `TeamManager::delete_team` instead of schema-level `ON DELETE CASCADE`.
   - Current schema does not use cascading FKs for Team tables.
   - Explicit delete order preserves FK safety and keeps migration scope minimal.
2. Return deleted `TeamDefinitionRecord` from delete API.
   - Keeps response format consistent with existing `apiFetch` JSON handling.
   - Simplifies UI reconciliation after delete.
3. UI confirmation guard before delete.
   - Deletion is irreversible at runtime scope.
   - A lightweight browser confirm avoids accidental deletion.
4. UI state reconciliation after delete.
   - Remove deleted team from list.
   - Remove deleted team's runs from local cache.
   - Clear stale run selection and fall back to next available team.

## Validation

Backend tests:

- `src/api/teams/tests_core.rs`
  - `teams_api_delete_team_cascades_related_run_data`
  - `teams_api_delete_team_returns_not_found_when_missing`
- `src/api/teams/tests_router.rs`
  - `teams_router_http_contract` now includes delete success + delete missing + post-delete lookup checks
- `src/api/openapi.rs`
  - OpenAPI test now checks `paths./api/teams/{id}.delete`

Frontend tests:

- `web/tests/e2e/team_page.e2e.ts`
  - `team list supports deleting selected team`

Manual checks recommended:

- Create two teams in `/teams`, delete the selected one, verify next team becomes active.
- Confirm deleted team cannot be fetched by id and deleted team's runs are no longer visible.
