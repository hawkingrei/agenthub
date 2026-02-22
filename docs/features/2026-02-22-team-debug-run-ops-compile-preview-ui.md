# Team Debug Run Ops Compile Preview UI

## Background

Team backend now exposes `compile_run_preview` for deterministic translation from
main-task discussion artifacts to run payload. The Team UI still required manual
copying from API output into `Create Run`, which made chat-first preview usage
slow and error-prone.

## Scope

- Add Team API client types and endpoint binding for:
  - `POST /api/teams/:id/main_tasks/:main_task_id/compile_run_preview`
- Add Debug Run Ops UI section in Team page for compile preview:
  - input `main_task_id`
  - optional `context_id` override
  - trigger `Compile Preview`
  - render compiled preview JSON
- Add direct actions from preview:
  - `Use Payload in Create Run` (fills `context_id` + run input JSON)
  - `Create Run from Preview` (calls existing create-run API directly)
- Add Playwright case for compile-preview run-ops path.

## Key Decisions

1. Keep compile flow in `Debug -> Run Ops` for now:
   - preserves existing conversation-first migration path
   - avoids forcing immediate main-surface UX switch before full E2E rollout
2. Keep preview application explicit:
   - users can inspect compiled payload before run creation
   - payload adoption and run creation are separate buttons
3. Keep fallback compatibility:
   - existing manual `Create Run` flow remains available unchanged.

## Validation

Executed locally:

```bash
cd web
npm run lint
npm run build
npm run test -- src/pages/team_page.runs.test.ts src/pages/team_panels.test.tsx src/pages/team/state.test.ts
```

All passed.

Playwright command added and attempted:

```bash
cd web
npm run e2e -- tests/e2e/team_page.e2e.ts -g "team debug run ops compiles main task preview and applies payload to create-run form"
```

Local execution in the current environment is still blocked: sandboxed runs hit
`listen EPERM` for loopback port binding, and escalated retry timed out waiting
for Playwright `webServer` readiness. This remains a follow-up verification item
in `docs/todo.md`.

## Follow-up

- Run the new Playwright case in CI (or clean local runtime) and record run ID.
- Extend from Debug entry to full chat-first primary-surface flow after the
  broader `main task -> negotiation -> compile -> execute -> synthesize` E2E
  coverage is complete.
