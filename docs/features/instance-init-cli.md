# Instance Init And First-Run Setup

## Goal

Provide a first-run CLI bootstrap path for local operators who install
`agenthub` through Homebrew or run the binary directly and do not yet have a
`~/.agenthub/config.toml`, plus a narrow web first-run surface for instances
that have started without a root operator.

The initial slice should make the instance identity explicit:

- `main` control plane
- `node` remote execution node

This closes the current gap where `brew services start ...` can launch AgentHub
without any explicit guidance about which runtime role the machine should take,
and where a first browser visit only showed a login form even when the instance
needed root bootstrap.

## Scope

The new `agenthub init` command should:

1. run as an interactive terminal wizard
2. write `~/.agenthub/config.toml`
3. ask for the minimal role-specific fields needed by the current runtime
4. generate config that matches the existing `AppConfig` schema exactly

The first slice covers:

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

## Non-Goals

This slice does not attempt to solve every first-run setup concern.

It deliberately does **not**:

- add Homebrew-specific launch-time prompts
- invent new provider config keys that do not exist in `AppConfig`
- configure ACP provider API base URLs or API keys
- configure server role, internal gRPC, or provider credentials from the browser

Provider/runtime credentials can be added in a later slice once they have a
reviewed config contract.

## Architecture

There are two first-run entry points:

- `agenthub init` writes the local instance config before the server starts.
- The web login view surfaces root-account bootstrap when `/api/auth/status`
  reports `root_initialized: false`.

The browser path reuses the existing root registration API. It is not a
configuration writer, so it cannot change the instance role, internal gRPC
settings, or provider credentials. Those values remain operator-managed through
the config file until a separate provider config contract exists.

## Contracts

- A running instance with no root operator presents a distinct first-run setup
  state instead of a generic login-only form.
- The first-run web state creates only the initial root operator.
- The first-run web state must state that server role and provider credentials
  remain outside the browser bootstrap path.
- Once a root operator exists, the login view returns to normal login language
  and does not keep first-run setup guidance visible.
- While root status is loading, the login view shows a neutral setup check
  state. If the status endpoint cannot be loaded, the browser falls back to the
  normal login shell instead of staying permanently blocked.

## Interaction Model

`agenthub init` should behave like a narrow bootstrap assistant:

- require an interactive terminal
- warn before overwriting an existing config file
- keep defaults explicit in prompts
- emit a concrete post-write note about what was and was not configured

If the operator chooses `node`, the command should also remind them that
provider-specific ACP credentials are still out of scope for this command.

The web surface should behave like a narrow root bootstrap panel:

- show first-run status only while no root operator exists;
- collect username, password, and display name for the first root account;
- keep role/provider configuration guidance explicit but non-interactive.

## Validation

Focused validation for this slice:

- root CLI parsing recognizes `agenthub init`
- `agenthub init --help` renders correctly
- generated TOML matches the current config schema for `main`
- generated TOML matches the current config schema for `node`
- interactive answer collection covers:
  - `main` without internal gRPC
  - `node` with required internal gRPC/bootstrap fields
- web login rendering shows first-run setup language only when root is missing
- web login rendering keeps role/provider configuration outside root bootstrap
- web login rendering covers setup loading while root status is unknown
- auth status failure falls back to the normal login shell

## Operational Notes

Operators that start the service before running `agenthub init` can still create
the first root account from the browser. They must still use the config file for
server role and provider/runtime credentials.

## Open Risks

- Provider API base URLs and API keys still need a reviewed config contract
  before they become first-class setup inputs. They are intentionally not
  first-run web setup inputs.
- Browser-side role/internal gRPC setup remains out of scope until there is a
  safe write path for instance configuration.

## Source Journals

- [../journal/2026-07-20-first-run-web-setup-surface.md](../journal/2026-07-20-first-run-web-setup-surface.md)
- [../journal/2026-07-22-first-run-setup-closeout.md](../journal/2026-07-22-first-run-setup-closeout.md)
