# PR 119 Review Follow-ups

## Summary

Addressed the remaining PR #119 review comments with three focused fixes:

1. Narrowed the `/teams` runtime status helper input type without rejecting callers that include `member_id`.
2. Updated the Rust team API test fixture to resolve the real `agenthub` binary path instead of depending on the test harness executable path.
3. Made the Playwright `/teams` runtime mock stateful so `/api/teams/:id/runtime` reflects `start` and `stop` transitions consistently.

## Validation

- `cargo test resolve_test_agenthub_binary_path_prefers_real_binary -- --nocapture`
- `cd web && npx vitest run src/pages/team/page_helpers.test.ts --pool=threads --maxWorkers=1`
- `cd web && npm run lint`

## Notes

- The new Playwright runtime-control coverage was added to `web/tests/e2e/team_page.e2e.ts`.
- Local Playwright execution remains blocked in the current macOS sandbox because Chromium cannot complete Mach port bootstrap; the mock logic was still linted and kept in-tree for CI coverage.
