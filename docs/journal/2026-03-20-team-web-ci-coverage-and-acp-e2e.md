## Summary

Stabilized the remaining `Web` and `Web E2E` failures on `fix/team-runtime-bugfixes`.

## Changes

- Switched the web coverage script to `vitest --pool=forks --maxWorkers=1` so CI no longer relies on worker threads for the full coverage run, which had been dying with `ERR_WORKER_OUT_OF_MEMORY`.
- Updated the Team Playwright ACP send step to target the actual Input Dock button accessibility contract (`Send input`) instead of the channel send button label (`Send`).

## Validation

- `cd web && npm run lint -- tests/e2e/team_page.e2e.ts package.json`
- `cd web && npm run test:coverage` (local run no longer reproduced the prior `ERR_WORKER_OUT_OF_MEMORY` thread-pool failure during suite execution; GitHub CI remains the final source of truth for full completion)
