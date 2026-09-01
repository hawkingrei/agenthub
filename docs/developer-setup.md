# Developer Setup

This guide covers local source development for AgentHub contributors.

## Requirements

- Rust 1.96.0
- Node.js 20+
- Bazel / Bazelisk (optional, for Bazel-driven checks)
- Git

## Repository Layout

```text
agenthub/
  src/                    # Rust server and runtime wiring
  crates/                 # Rust domain crates
  web/                    # Vite + React frontend
  userdocs/               # Docusaurus user documentation site
  proto/                  # Protobuf schema
  tests/                  # Integration and blackbox tests
  docs/                   # Internal engineering docs and journals
  skills/                 # Team/agent runtime skill definitions
  agenthub-codex-acp/     # Codex ACP runtime library embedded in the daemon
  crates/agenthub-acp-adapter/ # Generic ACP provider adapter library
  crates/agenthub-daemon/ # Long-running server and built-in provider worker binary
```

## Local Startup

### Install Web Dependencies

```bash
npm --prefix web ci
```

### Create A Minimal Config

AgentHub reads config from `~/.agenthub/config.toml`.

```toml
[server]
listen = "127.0.0.1:8080"

[worktree]
default_root = "~/.agenthub/worktrees"

[history]
event_retention_days = 5
vacuum_on_cleanup = false
```

Runtime state defaults to `~/.agenthub/`.

### Start Locally

```bash
make run
```

`make run` builds the embedded web UI and both AgentHub executables, stages the
checksum-pinned Codex Code Mode Host beside the debug daemon, then starts
`agenthubd`. The first run requires network access to download the host artifact
matching AgentHub's pinned Codex revision.

Open `http://localhost:8080`.

AgentHub builds and owns two native executables. Runtime helpers and built-in
provider workers are subcommands or multicall modes of `agenthub` and
`agenthubd`; the upstream `codex-code-mode-host` remains a separate
version-matched runtime companion:

```bash
cargo run -- actor --help
cargo run -- actor team-members --help
```

## Common Commands

```bash
# Run AgentHub server
make run

# Rust tests
cargo test

# Frontend unit tests
npm --prefix web run test

# Frontend lint
npm --prefix web run lint

# Frontend build
npm --prefix web run build

# Playwright E2E
npm --prefix web run e2e

# Bazel checks
bazel build //...
bazel test //...
bazel coverage --combined_report=lcov --test_output=errors //crates/agenthub-text:agenthub_text_tests
```

## Recommended Pre-PR Checks

```bash
# Rust + proto guard
cargo test
make proto-check

# Web checks
npm --prefix web run lint
npm --prefix web run test:coverage

# User docs
npm --prefix userdocs ci
npm --prefix userdocs run build

# Optional: E2E smoke
npm --prefix web run e2e -- tests/e2e/app.e2e.ts --project=chromium
```

## User Docs Preview

```bash
npm --prefix userdocs ci
npm --prefix userdocs run start
npm --prefix userdocs run build
```

## CI Pipelines

- `Rust`: cargo check + coverage (`rust-cargo.lcov`) + Codecov upload
- `Clippy`: `cargo clippy --workspace --all-targets -- -D warnings`
- `Web`: lint + unit coverage + build + Codecov upload
- `Web E2E`: Playwright coverage + Codecov upload
- `Bazel`: split `Bazel Build`, `Bazel Test (Root)`, `Bazel Test (Crates)`, and
  `Bazel Coverage` jobs, with `bazel.lcov` uploaded to Codecov and an aggregate
  `Bazel Build and Test` gate
- `User Docs`: Docusaurus build validation
