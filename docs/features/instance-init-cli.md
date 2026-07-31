# Instance First-Run Setup

## Problem

AgentHub needs a first-run path that makes instance ownership and runtime role
explicit before normal operation.

Two bootstrap surfaces are intentionally separate:

- the local `agenthub init` CLI writes instance configuration for operators who
  install `agenthub` through Homebrew or run the binary directly
- the web login surface creates the first root account when the backend reports
  that the instance has no root user

This closes the gap where `brew services start ...` can launch AgentHub without
clear role guidance, while also giving browser-first operators a reviewed setup
surface for the root account.

## Scope

The `agenthub init` command should:

1. run as an interactive terminal wizard
2. write `~/.agenthub/config.toml`
3. ask for the minimal role-specific fields needed by the current runtime
4. generate config that matches the existing `AppConfig` schema exactly

The CLI slice covers:

- `[server]`
  - `role`
  - `listen` for `main`
  - `node_id` for `node`
- `[internal_grpc]`
  - `enabled`
  - `listen`
  - `security.mode`
  - `security.cert_dir`
  - `auth.shared_secret`
  - `auth.issuer`
  - `auth.audience`
  - `bootstrap.token`

The web slice covers:

- showing a first-run setup surface when `rootInitialized === false`
- explaining that the browser step creates the first root/operator account
- keeping normal login compact once a root account already exists
- pointing instance role, listener, and provider credential setup back to the
  local instance configuration path

## Non-Goals

The first-run setup contract deliberately does **not**:

- add Homebrew-specific launch-time prompts
- let the web UI write `~/.agenthub/config.toml`
- invent provider config keys that do not exist in `AppConfig`
- configure ACP provider API base URLs or API keys from the root bootstrap form
- merge local instance configuration and root-account registration into one
  wizard

Provider API base URLs and API keys remain post-init operator guidance for now.
They should become first-class config only after a separate reviewed config
contract defines schema keys, storage expectations, redaction behavior, and
migration rules.

## Architecture

`agenthub init` owns local instance configuration. It is the canonical place to
choose whether the machine runs as a `main` control plane or a `node` remote
execution host, and it writes the existing TOML schema without introducing a
parallel config model.

The web login route owns root-account bootstrap. It consumes the existing auth
status signal, renders the first-run state when `rootInitialized` is false, and
submits the existing root registration action. It does not perform filesystem
writes or mutate runtime/provider configuration.

## Contracts

- `agenthub init` requires an interactive terminal.
- `agenthub init` warns before overwriting an existing config file.
- `agenthub init` keeps prompt defaults explicit.
- `agenthub init` emits a concrete post-write note about what was and was not
  configured.
- When the operator chooses `node`, the CLI reminds them that provider-specific
  ACP credentials are still out of scope.
- The web login surface renders first-run setup copy only when the instance has
  no root account.
- The web first-run action creates the root account through the existing
  registration path.
- The normal login state must not show first-run setup copy after the root
  account exists.

## Validation Matrix

Focused validation for this contract:

- root CLI parsing recognizes `agenthub init`
- `agenthub init --help` renders correctly
- generated TOML matches the current config schema for `main`
- generated TOML matches the current config schema for `node`
- interactive answer collection covers:
  - `main` without internal gRPC
  - `node` with required internal gRPC/bootstrap fields
- SSR rendering for `LoginView` shows the reviewed first-run setup surface when
  `rootInitialized` is false
- SSR rendering for `LoginView` keeps normal login free of first-run setup copy
  when `rootInitialized` is true

## Operational Notes

Operators should run `agenthub init` before service startup when they need to
choose instance role or internal gRPC settings explicitly. If the backend starts
without a root user, the browser setup surface is still valid for creating the
first root account, but it is not a substitute for instance configuration.

Provider credentials should continue to be documented as operator-managed
configuration until the project accepts a first-class provider config schema.

## Open Risks

- The provider credential story is still guidance-only. A future config contract
  must define schema, secret handling, redaction, and migration behavior before
  the web UI can manage it.
- The web first-run surface depends on the auth status endpoint returning
  `rootInitialized` accurately.
- Operators can still start a service before running `agenthub init`; service
  manager packaging should keep pointing operators at this first-run contract.

## Source Journals

- [2026-07-19 first-run web setup surface](../journal/2026-07-19-first-run-web-setup-surface.md)
