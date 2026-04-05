# PR 296 Review Follow-ups

## Summary

- Stopped `apiFetch` from forwarding the internal-only `networkRetry` field to `fetch`.
- Prevented HTTP status errors from being retried as if they were transient network failures.
- Tightened the Team page E2E selector helper so team-detail readiness confirms the selected team
  through the stable `Open selected team menu` trigger instead of the sidebar selection marker.
- Updated ACP long-text rendering coverage so the `Content` terminal tone assertion no longer
  depends on class ordering.

## Validation

- `cd web && npx vitest run src/api.test.ts --pool=threads --maxWorkers=1`
- `cd web && npx vitest run src/acp_conversation_render.test.tsx --pool=threads --maxWorkers=1`
- `cd web && npm run lint -- src/api.ts src/api.test.ts tests/e2e/team_page.e2e.ts`
- `make build-web`
