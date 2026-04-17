# Web Hide Settled Permission Reviews

This change hides settled ACP permission review items from the main UI surfaces instead of keeping
them visible as compact status cards.

## What changed

- Team task conversation now skips permission review cards when the effective status is:
  - `timeout`
  - `responded` with a non-empty `selected_option_id` (approved)
- The same hidden-status rule now applies to ACP debug permission history so the debug tab does not
  keep showing approved or timed-out review items.
- Pending reviews still render and still trigger the fallback human-review tone.
- Cancelled reviews remain visible because they do not carry an approval option selection and can
  still be useful as audit context.
- Review follow-up tightened the helper boundaries:
  - `filterPermissionsForAgent(...)` once again scopes by `agent_id` only
  - visibility filtering now lives in `filterVisiblePermissionRecords(...)` and
    `filterVisiblePermissionsForAgent(...)`
- Team task pending-review detection now reuses `resolvePermissionCardStatus(...)` so timeout cards
  derived from `payload.reason` cannot slip back into the pending bucket before polling catches up.

## Intent

- Keep the Team thread focused on active work that still requires operator action.
- Remove stale review noise after approval or timeout.
- Keep filtering consistent across Team task UI and ACP debug history.

## Validation

- `cd web && npm run test -- vite.config.test.ts src/pages/team_panels.test.tsx src/acp_debug.test.tsx src/app.permission_scope.test.ts`

## MCP verification

- Chrome DevTools MCP baseline and regression checks were both attempted.
- Both attempts failed before inspection with `Transport closed`, so this change only has unit-test
  validation and no browser-side MCP evidence.
