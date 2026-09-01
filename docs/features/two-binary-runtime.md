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
- Delegate Codex execution and helper ownership to a separately installed official Codex CLI.
- Preserve existing stored agent records that use the former bare built-in adapter commands.

## Non-Goals

- Bundling external provider CLIs such as Codex, Gemini, or Kimi.
- Treating npm launch shims, maintainer scripts, or systemd units as native binaries.
- Rewriting custom absolute executable paths supplied by operators.
- Changing the ACP wire protocol or provider session behavior.

## Architecture

AgentHub builds and owns exactly two native executables:

| Executable | Ownership |
| --- | --- |
| `agenthub` | User-facing administration and actor CLI. A no-argument invocation remains a compatibility launcher for the sibling daemon. |
| `agenthubd` | Long-running main/node server plus internal stdio provider worker modes. |

Codex-backed sessions require the compatible official Codex CLI to be installed separately.
`agenthubd acp codex` remains a mode of the same daemon file, but it launches
`codex app-server --stdio` as a child process and translates between Agent Client Protocol and the
Codex app-server protocol. The official Codex installation owns its Code Mode Host, sandbox, exec,
filesystem, patch, authentication, configuration, and upgrade coupling.

The daemon dispatches these process roles before starting an async runtime:

1. no subcommand: start the configured main or node server role;
2. `acp codex`: run the built-in Codex ACP stdio worker;
3. `acp claude`: run the built-in Claude ACP stdio worker.

Provider workers remain separate child processes for isolation, but every built-in worker executes the
same `agenthubd` file. The Codex worker then creates an official Codex child for each live app-server
connection. This preserves one AgentHub daemon artifact while keeping the provider runtime boundary
explicit.

## Contracts

### Artifact Contract

- AgentHub Cargo metadata and Bazel native binary rules expose only `agenthub` and `agenthubd`.
- The repository contains no excluded standalone Cargo manifest that can redefine either executable.
- Release archives publish one archive for each executable and target.
- Release archives, Debian packages, and npm platform packages do not bundle Codex or Codex helper
  executables.
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

- Normal daemon startup does not launch Codex or mutate PATH for helper aliases.
- Codex worker startup resolves the configured runtime path, executes `codex --version`, and fails
  with an actionable error unless the supported official version is present.
- Each Codex session starts the resolved executable as `codex app-server --stdio` in the session
  workspace and performs the official `initialize` / `initialized` JSONL handshake before sending
  thread requests.
- The Codex worker forwards the effective ACP-provided MCP server map through per-thread app-server
  configuration so Team mailbox and other managed tools survive the process boundary.
- `[codex_acp].runtime_binary` configures the official Codex executable. The default is `codex` from
  the daemon child's PATH; an absolute path is recommended for managed services.
- The daemon passes the sibling `agenthub` path explicitly to the Codex ACP adapter's actor
  integration. The adapter must not treat `current_exe()` (`agenthubd`) as the actor CLI.
- Proxy variables applied to the `agenthubd acp codex` child are inherited by the official Codex
  app-server child.
- AgentHub must not dispatch Codex argv0 helper modes. Helper discovery and compatibility belong to
  the installed Codex distribution.

## Validation Matrix

| Boundary | Required evidence |
| --- | --- |
| AgentHub artifact count | `cargo metadata --locked --format-version 1 --no-deps` lists only `agenthub` and `agenthubd`; source scan finds two AgentHub Bazel `rust_binary` rules. |
| Daemon dispatch | Unit tests cover default server mode and both built-in provider modes. |
| Stored command compatibility | Backend tests cover exact bare legacy rewrites, new provider detection, and non-rewrite of absolute custom paths. |
| Actor CLI identity | Unit tests prove daemon-to-sibling CLI resolution and explicit runtime propagation. |
| Codex runtime boundary | Focused tests use a fake installed executable to cover version preflight, command arguments, JSONL initialization, typed requests, and shutdown. |
| Packaging | Release workflow tests assert two AgentHub archive names and confirm no Codex helper is bundled. |
| Source development | Makefile contract tests confirm `make build` and `make run` build only AgentHub-owned executables. |
| Runtime | Focused Cargo and Bazel checks pass on the exact change head; release cross-builds remain a rollout gate. |

## Operational Notes

- Operators using Codex-backed agents must install the supported official Codex CLI and make it
  executable by the daemon service account.
- `agenthubd acp ...` is an internal subprocess interface. It is documented for diagnostics, not as a
  separately supervised service.
- External provider commands continue to be discovered through their existing configured path or PATH.
- Codex and its helpers are external provider artifacts and do not change the AgentHub executable
  count.
- Daemon-owned local children follow the process-tree and shutdown ordering contract in
  [Daemon Process Supervision](daemon-process-supervision.md).

## Open Risks

- Release cross-build and published archive evidence is required before removing all upgrade guidance
  for the former adapter executables.
- Third-party packaging that installs only the CLI must add the sibling daemon before adopting this
  contract and must document the external Codex prerequisite for Codex-backed agents.
- The app-server protocol is versioned with Codex. AgentHub intentionally fails closed on version
  skew until the pinned protocol types and supported runtime version are upgraded together.

## Source Journals

- [Two-binary runtime consolidation](../journal/2026-08-27-two-binary-runtime-consolidation.md)
- [Code Mode Host companion packaging (superseded)](../journal/2026-08-28-code-mode-host-companion.md)
- [ACP install and proxy recovery](../journal/2026-09-01-acp-install-proxy-recovery.md)
- [Release vendored OpenSSL and partial assets](../journal/2026-04-20-release-vendored-openssl-and-partial-assets.md)
