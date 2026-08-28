# Two-Binary Runtime

## Problem

The runtime historically exposed separate native executables for the control plane, the generic ACP
adapter, and the Codex ACP compatibility entrypoint. The adapter processes are implementation details,
not independently operated services, and publishing them separately expands installation, version
skew, and discovery failure modes.

## Scope

- Define the two native executables built and owned by AgentHub.
- Separate user-facing CLI behavior from long-running daemon ownership.
- Embed built-in Codex and Claude ACP worker modes in the daemon executable.
- Preserve Codex multicall behavior without publishing additional AgentHub executables.
- Package the upstream Code Mode Host as an explicitly allowed, version-matched runtime companion.
- Preserve existing stored agent records that use the former bare built-in adapter commands.

## Non-Goals

- Bundling external provider CLIs such as Gemini or Kimi.
- Treating npm launch shims, maintainer scripts, or systemd units as native binaries.
- Rewriting custom absolute executable paths supplied by operators.
- Changing the ACP wire protocol or provider session behavior.

## Architecture

AgentHub builds and owns exactly two native executables:

| Executable | Ownership |
| --- | --- |
| `agenthub` | User-facing administration and actor CLI. A no-argument invocation remains a compatibility launcher for the sibling daemon. |
| `agenthubd` | Long-running main/node server plus internal stdio provider worker and Codex multicall modes. |

Official packages also contain `codex-code-mode-host`. It is an upstream runtime companion, not an
AgentHub command or service. AgentHub downloads the official artifact matching the pinned Codex
dependency and verifies a checked-in SHA-256 digest before packaging it.

The daemon dispatches these process roles before starting an async runtime:

1. no subcommand: start the configured main or node server role;
2. `acp codex`: run the built-in Codex ACP stdio worker;
3. `acp claude`: run the built-in Claude ACP stdio worker;
4. Codex argv0 or hidden argv1 helper mode: delegate to the pinned Codex multicall dispatcher.

Provider workers remain separate child processes for isolation, but every built-in worker executes the
same `agenthubd` file. Codex-created sandbox, exec, filesystem, and patch helpers are temporary aliases
or re-executions of that file, not additional release artifacts. Code Mode is the exception: its V8
runtime remains in the separately built upstream companion process.

## Contracts

### Artifact Contract

- AgentHub Cargo metadata and Bazel native binary rules expose only `agenthub` and `agenthubd`.
- The repository contains no excluded standalone Cargo manifest that can redefine either executable.
- Release archives publish one archive for each executable and target.
- The `agenthubd` archive, Debian package, and npm platform package also contain the official
  `codex-code-mode-host` artifact matching the pinned Codex version and target.
- The companion download must fail closed on an unsupported target or SHA-256 mismatch.
- The default systemd unit executes `agenthubd` directly.

### CLI And Daemon Contract

- `agenthub init`, `agenthub doctor`, `agenthub actor`, and `agenthub migrate` remain CLI-owned.
- A no-argument `agenthub` invocation locates a sibling `agenthubd` first, then falls back to PATH.
- Daemon configuration, server roles, shutdown cleanup, and embedded web serving are daemon-owned.

### Provider Command Contract

- New built-in presets use `agenthubd acp codex` or `agenthubd acp claude`.
- The exact bare legacy commands `agenthub-acp` and `agenthub-codex-acp` are rewritten at spawn time
  to the corresponding daemon worker mode.
- Absolute or otherwise operator-supplied custom paths are not rewritten.
- Provider identification recognizes both the new daemon form and legacy forms for stored-data and UI
  compatibility.

### Codex Runtime Contract

- Codex worker startup runs through the pinned upstream argv0 dispatcher before any Tokio runtime is
  created.
- Normal daemon startup does not load Codex dotenv state or mutate PATH for helper aliases.
- The daemon passes the sibling `agenthub` path explicitly to the Codex runtime. The runtime must not
  treat `current_exe()` (`agenthubd`) as the actor CLI.
- The daemon archive and installed layouts place `codex-code-mode-host` beside `agenthubd` so Codex
  install-context discovery resolves the host without PATH-dependent fallback.
- `codex-code-mode-host` is not a daemon mode and must not be supervised as an AgentHub service.

## Validation Matrix

| Boundary | Required evidence |
| --- | --- |
| AgentHub artifact count | `cargo metadata --locked --format-version 1 --no-deps` lists only `agenthub` and `agenthubd`; source scan finds two AgentHub Bazel `rust_binary` rules. |
| Daemon dispatch | Unit tests cover default server mode, both provider modes, Codex helper aliases, and hidden helper arguments. |
| Stored command compatibility | Backend tests cover exact bare legacy rewrites, new provider detection, and non-rewrite of absolute custom paths. |
| Actor CLI identity | Unit tests prove daemon-to-sibling CLI resolution and explicit runtime propagation. |
| Companion provenance | Packaging tests assert the Codex version/revision coupling, supported target mapping, and pinned archive digests. |
| Packaging | Release workflow tests assert two AgentHub archive names; archive, Debian, and npm inspection confirm the companion is placed beside the daemon. |
| Runtime | Focused Cargo and Bazel checks pass on the exact change head; release cross-builds remain a rollout gate. |

## Operational Notes

- Operators should install the AgentHub executable pair and Code Mode Host companion from the same
  release package set.
- `agenthubd acp ...` is an internal subprocess interface. It is documented for diagnostics, not as a
  separately supervised service.
- External provider commands continue to be discovered through their existing configured path or PATH.
- Temporary Codex helper aliases do not change the AgentHub executable count.
- The Code Mode Host companion is allowed because it preserves V8 process isolation and avoids adding
  OpenAI's V8 build toolchain to AgentHub's Cargo and Bazel graphs.
- Daemon-owned local children follow the process-tree and shutdown ordering contract in
  [Daemon Process Supervision](daemon-process-supervision.md).

## Open Risks

- Release cross-build and published archive evidence is required before removing all upgrade guidance
  for the former adapter executables.
- Third-party packaging that installs only the CLI must add the sibling daemon and version-matched
  Code Mode Host companion before adopting this contract.
- The helper-mode recognizer is coupled to the pinned Codex revision and must be reviewed with every
  Codex upgrade.

## Source Journals

- [Two-binary runtime consolidation](../journal/2026-08-27-two-binary-runtime-consolidation.md)
- [Code Mode Host companion packaging](../journal/2026-08-28-code-mode-host-companion.md)
- [Release vendored OpenSSL and partial assets](../journal/2026-04-20-release-vendored-openssl-and-partial-assets.md)
