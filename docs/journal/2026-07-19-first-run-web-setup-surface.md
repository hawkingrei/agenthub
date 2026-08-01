# First-Run Web Setup Surface

## Summary

AgentHub now has a reviewed web first-run setup surface for the root account.
The stable first-run contract keeps local instance configuration in
`agenthub init` / `~/.agenthub/config.toml`, while the browser setup path only
bootstraps the first root operator.

## Background

The existing first-run contract covered `agenthub init`, but the web login route
already had a root registration path when `rootInitialized` was false. That made
the browser flow functional but under-specified: operators could create the root
account without seeing the boundary between account bootstrap and instance
configuration.

## Scope

This checkpoint covers:

- first-run setup copy on the login route when no root account exists
- normal login remaining compact after root bootstrap
- canonical documentation for the split between CLI instance configuration and
  browser root bootstrap
- a decision on provider API base URLs and API keys

## Key Decisions

- The web setup surface creates the first root/operator account through the
  existing registration action.
- The web setup surface does not write `~/.agenthub/config.toml`.
- `agenthub init` remains the canonical owner of server role, listener, internal
  gRPC, and node bootstrap settings.
- Provider API base URLs and API keys stay out of first-class config for now.
  They should move into config only after a reviewed schema defines keys,
  secret handling, redaction, and migration behavior.

## Validation

Focused checks for this slice:

```bash
cd web && npm exec vitest -- run src/routes/login_view.test.tsx
git diff --check
```

## Follow-Ups

- Define a dedicated provider runtime config contract before adding provider API
  base URLs or API keys to `AppConfig` or web setup.
- Keep packaging and service-manager docs pointing operators at
  `agenthub init` for runtime role configuration.
