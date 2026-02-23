# Auth Error Message Normalization

## Background

Login and join flows surfaced some backend failures as raw JSON strings wrapped by JavaScript `Error` text, for example:

- `Error: {"error":"user not found"}`

This is noisy for users and obscures the actionable message.

## Scope

- Frontend-only error-message normalization for auth/join and shared app actions.
- No backend API contract changes.
- No schema or migration changes.

## Key Decisions

1. Add a shared parser in `web/src/api.ts`:
   - `parseApiErrorMessage(err: unknown): string | null`
2. Parse JSON-shaped messages and extract `error` when present.
3. Update API fetch failure throw path to prefer parsed message text over raw JSON blob.
4. Replace direct `setError(String(err))` usages in app/join flows with:
   - `setError(parseApiErrorMessage(err) ?? String(err))`
5. Add focused unit coverage for parser behavior.

## Files Changed

- `web/src/api.ts`
- `web/src/app.tsx`
- `web/src/pages/join_page.tsx`
- `web/src/api_error_message.test.ts`
- `docs/todo.md`

## Validation

Run:

- `npm --prefix web run test -- src/api_error_message.test.ts src/create_agent_modal.test.tsx src/pages/team_panels.test.tsx`
- `npm --prefix web run lint`
- `npm --prefix web run build`

## Chrome DevTools MCP Notes

- Baseline connectivity re-validated for `https://agenthub.hawkingrei.com/` (`list_pages`, `new_page`, `take_snapshot`).
- This change targets local source behavior and requires post-deploy verification in real login flow to confirm the banner now displays plain message text.
