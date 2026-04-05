# PR 296 Review Follow-ups

## Summary

- Stopped `apiFetch` from forwarding the internal-only `networkRetry` field to `fetch`.
- Prevented HTTP status errors from being retried as if they were transient network failures.
- Tightened the Team page E2E selector helper so team-detail readiness also confirms the expected
  selected team entry.

## Validation

- `cd web && npx vitest run src/api.test.ts --pool=threads --maxWorkers=1`
- `cd web && npm run lint -- src/api.ts src/api.test.ts tests/e2e/team_page.e2e.ts`
- `make build-web`
