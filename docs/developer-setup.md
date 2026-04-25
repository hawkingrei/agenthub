# Developer Setup

This guide covers local source development for AgentHub contributors.

## Requirements

- Rust (stable)
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
  agenthub-codex-acp/     # Codex ACP integration workspace member
```

## Local Startup

### Install Web Dependencies

```bash
npm --prefix web ci
```

### Create A Minimal Config

AgentHub reads config from `~/.agenthub/config.toml`.

```toml
safe_paths = [
  "/home/foo",
  "/home/foo/projects"
]

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

`make run` builds the embedded web UI as part of the normal local startup path.

Open `http://localhost:8080`.

AgentHub stays single-binary. Runtime helpers such as the actor CLI are
subcommands of the same binary:

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
