# Team Chat-First / Create Flow E2E Hardening

## Background

Team roadmap still had pending verification for two tracks:

- chat-first golden path E2E (`task -> compile preview -> create run -> worker evidence -> final synthesis`)
- Team create-flow closure (`Guided Wizard` / `Manual Spec` entry behavior and forge modal layering)

The required scenarios already existed, but local verification remained fragile in sandboxed environments.

## Scope

- `web/playwright.config.ts`
  - Normalize Playwright `webServer` startup cwd by pinning it to the config directory (`web/`) so invocation path differences do not affect command resolution.
- `web/src/pages/team_page.tsx`
  - Hide the in-modal Agent Forge panel for manual-spec entry path after Mission Brief (`useSpecOverride`), so manual path stays focused on direct spec editing.
- `web/tests/e2e/team_page.e2e.ts`
  - Add explicit portal-layer assertion for Team Forge `Create Agent` modal (not nested inside Team Forge dialog).
  - Add explicit no-toggle/no-forge assertions for manual-spec flow in Launch stage.

## Key Decisions

1. Keep chat-first and create-flow as mock-driven Playwright coverage.
   - This remains deterministic and avoids backend-runtime scheduler coupling in UI regression tests.
2. Treat manual-spec path as direct spec editing flow.
   - Keep Mission Brief + Launch Team only, and remove in-modal forge affordance during manual mode.
3. Keep Playwright config portable.
   - `webServer` command is still `npm run dev`, but with explicit `cwd` pinned to the web package directory.

## Validation

Executed locally:

```bash
npm --prefix web run lint
npm --prefix web run test -- src/pages/team_panels.test.tsx src/pages/team_page.runs.test.ts
npm --prefix web run build
PLAYWRIGHT_NO_WEBSERVER=1 npm --prefix web run e2e -- tests/e2e/team_page.e2e.ts -g "team debug run ops compiles task preview and applies payload to create-run form|team chat-first path compiles preview, creates run, and captures worker plus final synthesis evidence|team forge manual spec mode skips leader/worker stages|team forge modal creates team with leader/worker presets"
```

Result:

- lint/build/tests passed
- targeted Playwright set passed (`4/4`)

## Follow-up

- Keep the existing TODO items for CI run-ID evidence (`chat-first` and `compile-preview` Playwright entries) until a CI run records workflow IDs.
