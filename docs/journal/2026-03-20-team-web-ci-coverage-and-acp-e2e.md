## Summary

Stabilized the remaining `Web` and `Web E2E` failures on `fix/team-runtime-bugfixes`.

## Changes

- Switched the web coverage script to `vitest --pool=forks --maxWorkers=1` so CI no longer relies on worker threads for the full coverage run, which had been dying with `ERR_WORKER_OUT_OF_MEMORY`.
- Split `team_page.smoke.test.tsx` out of the coverage run: the main coverage command now excludes that smoke file, and `npm run test:smoke` executes it separately without coverage so the smoke worker no longer blocks coverage completion during fork shutdown.
- Trimmed `team_page.smoke.test.tsx` back to true route-render smoke coverage only; the prior ACP interaction assertion already lives in `team_panels.test.tsx`, so the smoke test no longer mounts extra client-side interaction state that kept fork workers alive.
- Updated the Team Playwright ACP send step to target the actual Input Dock button accessibility contract (`Send input`) instead of the channel send button label (`Send`), and changed the assertion to poll until the mocked Team mailbox send request is observed before checking payload contents.
- Corrected the Team chat-first E2E expectation to follow the real Agent ACP transport contract: the selected Team agent sends through `/api/agents/:id/input`, not Team mailbox `messages/send`, so the test now asserts the `api.sendInput` payload instead of an obsolete run-mailbox send mock.

## Validation

- `cd web && npm run lint -- tests/e2e/team_page.e2e.ts package.json`
- `cd web && npm run test:coverage` (local run no longer reproduced the prior `ERR_WORKER_OUT_OF_MEMORY` thread-pool failure during suite execution; GitHub CI remains the final source of truth for full completion)
- `cd web && npm run test:smoke`
