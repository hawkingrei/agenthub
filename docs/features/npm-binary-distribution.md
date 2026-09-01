# npm Binary Distribution Specification

## Problem

AgentHub release automation currently publishes GitHub release archives for Rust binaries, but
there is no supported `npm` installation path for the CLI.

That blocks the common operator flow of:

1. install the CLI with `npm install -g`
2. run `agenthub` immediately
3. stay aligned with GitHub release tags for versioning

## Scope

- npm distribution for the `agenthub` CLI and its sibling `agenthubd`
- package naming under the `@linkerdog` scope
- release-time publish automation
- supported platform contract for npm-distributed binaries

## Non-Goals

- Publishing the `web` SPA to npm
- Publishing `userdocs` to npm
- Adding Windows packages in this slice
- Replacing GitHub release archives as a distribution channel

## Architecture

### Package Layout

The npm distribution uses one wrapper package plus platform-specific binary packages:

- `@linkerdog/agenthub`
- `@linkerdog/agenthub-darwin-arm64`
- `@linkerdog/agenthub-linux-arm64`
- `@linkerdog/agenthub-linux-x64`

Repository package skeletons live under `build/npm/` because they are release/distribution assets,
not runtime app packages.

The main package exposes the `agenthub` executable through a small Node launcher that resolves the
correct optional dependency for the current platform. Each platform package places `agenthubd` next
to the CLI so the launcher can start the service through the normal sibling lookup contract. It also
contains the version-matched official `codex-code-mode-host` companion executable so Codex can resolve
the host through its expected sibling executable name.

### Binary Packaging Contract

- platform packages contain the native `agenthub` and `agenthubd` files from one release build
- platform packages contain the checksum-pinned official `codex-code-mode-host` release artifact that
  matches AgentHub's pinned Codex dependency
- the wrapper package contains the executable shim and declares platform packages as
  `optionalDependencies`
- release-time publish reuses the same Rust build artifacts already produced for GitHub releases
- npm package version must match the release semver version without the `v` prefix
- the initial package skeletons in-repo are pinned to `0.0.3`; release automation still rewrites
  versions from the active release tag before publish

### Release Contract

- GitHub release flow remains the source of truth for versioned CLI distribution
- npm publish runs only for semver-compatible release versions
- preview or non-semver release tags skip npm publish cleanly
- npm publish uses the repository secret `NPM_TOKEN`

## Contracts

### Supported npm Targets

The initial npm release surface supports:

- `darwin/arm64`
- `linux/arm64`
- `linux/x64`

Unsupported targets must fail with a clear runtime message from the wrapper package.

### Scope Contract

- all npm packages publish under `@linkerdog`
- the main public install target is `@linkerdog/agenthub`
- platform packages stay implementation detail packages and should not be the primary install path

## Validation Matrix

- Node unit coverage for platform package resolution in the wrapper launcher
- workflow dry validation by checking staged package version rewriting and artifact extraction logic
- staged platform-package inspection for both AgentHub executables and the Code Mode Host companion
- manual release verification:
  - semver tag publishes wrapper + platform packages
  - preview tag skips npm publish without failing the overall release flow

## Operational Notes

- repository maintainers must provide `NPM_TOKEN` with publish access to the `@linkerdog` scope
- package versions are derived from the release tag without the leading `v`
- GitHub release archives remain available for direct binary download even when npm publish is used

## Open Risks

- Published package inspection is still required for the first release that adopts the complete
  platform package with both AgentHub executables and the Code Mode companion.

## Source Journals

- [Two-binary runtime consolidation](../journal/2026-08-27-two-binary-runtime-consolidation.md)
- [Code Mode Host companion packaging](../journal/2026-08-28-code-mode-host-companion.md)
- [ACP install and proxy recovery](../journal/2026-09-01-acp-install-proxy-recovery.md)
