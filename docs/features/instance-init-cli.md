# Instance Init CLI

## Goal

Provide a first-run CLI bootstrap path for local operators who install
`agenthub` through Homebrew or run the binary directly and do not yet have a
`~/.agenthub/config.toml`.

The initial slice should make the instance identity explicit:

- `main` control plane
- `node` remote execution node

This closes the current gap where `brew services start ...` can launch AgentHub
without any explicit guidance about which runtime role the machine should take.

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

- introduce a web first-run setup wizard
- add Homebrew-specific launch-time prompts
- invent new provider config keys that do not exist in `AppConfig`
- configure ACP provider API base URLs or API keys

Provider/runtime credentials can be added in a later slice once they have a
reviewed config contract.

## Interaction Model

`agenthub init` should behave like a narrow bootstrap assistant:

- require an interactive terminal
- warn before overwriting an existing config file
- keep defaults explicit in prompts
- emit a concrete post-write note about what was and was not configured

If the operator chooses `node`, the command should also remind them that
provider-specific ACP credentials are still out of scope for this command.

## Validation

Focused validation for this slice:

- root CLI parsing recognizes `agenthub init`
- `agenthub init --help` renders correctly
- generated TOML matches the current config schema for `main`
- generated TOML matches the current config schema for `node`
- interactive answer collection covers:
  - `main` without internal gRPC
  - `node` with required internal gRPC/bootstrap fields
