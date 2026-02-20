# Web E2E Mailbox Tab And Coverage Merge Fix

## Background

After recent Team UI migration, `web` Playwright E2E showed two regressions:

1. Team mailbox interaction case failed because `Mailbox` tab click was intercepted by
   the `context_id` input from `Create / Load Run` section.
2. Coverage mode (`e2e:coverage`) logged repeated
   `TypeError: The URL must be of scheme file` from `ast-v8-to-istanbul`, producing
   `Unknown%` coverage summary.

## Scope

- `web/src/styles.css`
- `web/tests/e2e/coverage.ts`
- `web/tests/e2e/coverage_merge.ts`
- `docs/todo.md`

## Key Decisions

1. Stabilize shared form-row layout to avoid flex overflow side effects:
   - add `min-width: 0` for `.form-row`
   - enforce flexible shrinking for row inputs (`input/textarea/select`)
   - keep action buttons fixed-width (`flex: 0 0 auto`)
2. Protect Team tab switcher clickability in stacked cards:
   - add top spacing to `.tab-bar`
   - set explicit stacking order (`position: relative; z-index: 2`)
3. Make E2E coverage merge compatible with modern `ast-v8-to-istanbul` behavior:
   - normalize coverage URL to `file://` via `pathToFileURL(filePath).href`
4. Skip non-script sources in E2E coverage map resolution:
   - limit to JS/TS-like suffixes (`js/jsx/ts/tsx/mjs/cjs/mts/cts`)

## Validation Evidence (2026-02-20)

- Command:
  - `npm --prefix web run e2e -- tests/e2e/team_page.e2e.ts -g "team mailbox IM mode supports conversation focus, unread, auto-follow and advanced controls"`
- Result:
  - Passed after style fixes.

- Command:
  - `npm --prefix web run e2e:coverage -- tests/e2e/team_page.e2e.ts -g "team mailbox IM mode supports conversation focus, unread, auto-follow and advanced controls"`
- Result:
  - Passed with numeric coverage summary; URL-scheme merge errors removed.

- Command:
  - `npm --prefix web run e2e`
- Result:
  - `16 passed`.

- Command:
  - `npm --prefix web run e2e:coverage`
- Result:
  - `16 passed` with numeric coverage summary.

- Command:
  - `npm --prefix web run test -- tests/e2e/coverage_merge.test.ts`
- Result:
  - `5 passed`.

- Command:
  - `npm --prefix web run build`
- Result:
  - Vite production build passed.
