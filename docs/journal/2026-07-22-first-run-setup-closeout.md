# First-Run Setup Closeout

## Summary

The first-run setup P1 is closed as a narrow root-account bootstrap surface.
`agenthub init` remains the instance configuration writer, while the browser
surface creates the first root operator only.

## Background

The open TODO asked to extend first-run setup beyond `agenthub init`, add a
reviewed web setup surface, and decide whether provider API base URLs or API
keys should become first-class setup inputs.

## Scope

- Keep the browser first-run panel limited to root-account initialization.
- Keep runtime role, internal gRPC, and provider credential setup in
  operator-managed configuration.
- Treat provider API base URLs and API keys as a separate future config
  contract, not as fields in the first-run browser form.

## Key Decisions

- Provider API base URLs and API keys should not become first-class inputs in
  the first-run web setup surface until AgentHub has a reviewed provider config
  schema and safe server-side write path.
- The existing login-view first-run tests are the regression boundary for the
  browser bootstrap contract.
- The first-run setup contract is now captured in
  `docs/features/instance-init-cli.md`; remaining provider configuration work
  should be tracked as a dedicated provider/runtime config item if it becomes
  active.

## Validation

```bash
cd web && npm exec vitest -- run src/routes/login_view.test.tsx
cargo test --lib init_cli
```

## Follow-Ups

- No active first-run P1 remains.
- Open a separate provider/runtime config contract before adding provider API
  base URL or API key inputs to any setup surface.
