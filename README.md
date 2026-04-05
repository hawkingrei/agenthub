# AgentHub

AgentHub is a single-binary control plane for long-lived AI agents.
It combines a Rust backend, an embedded React web UI, ACP-based structured
output rendering, SQLite-backed persistence, multi-agent Team orchestration,
and optional remote execution nodes.

Quick links: [Docs Site](https://doc.agenthub.hawkingrei.com/) ·
[Why AgentHub](#why-agenthub) · [Quick Start](#quick-start) ·
[Remote Agent Nodes](#remote-agent-nodes-optional) ·
[Architecture At A Glance](#architecture-at-a-glance) ·
[Documentation Map](#documentation-map) · [Development](#development)

## Why AgentHub

Most agent tools are optimized for a single terminal session. AgentHub is built
for the operational side of agent workflows:

- Keep agent sessions alive after the browser tab closes
- Inspect structured ACP timelines instead of raw terminal scrollback
- Run leader/worker Team workflows with shared coordination primitives
- Route execution to remote Agent Nodes over internal gRPC when one machine is
  not enough
- Keep audit history, runtime state, and operator control in one place

## What You Can Do

- Create, start, stop, reconnect, and delete agents
- View ACP events such as messages, plans, tool calls, command output, and
  debug streams
- Run multi-agent Team workflows with leader/worker coordination
- Install AgentHub as an app-like PWA shell while still picking up fresh web
  deploys on refresh
- Register remote execution nodes and start agents on those nodes
- Persist session history and operational records in SQLite
- Receive completion notifications in the web UI

## Quick Start

### Requirements

- Rust (stable)
- Node.js 20+
- Bazel / Bazelisk (optional, for Bazel-driven checks)

### Start Locally

```bash
# 1) Install web dependencies
npm --prefix web ci

# 2) Start AgentHub
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

## Minimal Configuration

AgentHub reads config from `~/.agenthub/config.toml`.

```toml
safe_paths = [
  "/home/foo",
  "/home/foo/projects"
]

[server]
listen = "0.0.0.0:8080"

[worktree]
default_root = "~/.agenthub/worktrees"

[history]
event_retention_days = 5
vacuum_on_cleanup = false
```

Runtime state defaults to `~/.agenthub/`.

## Remote Agent Nodes (Optional)

If you want remote execution, run the same `agenthub` binary on the remote
machine and enable internal gRPC on both the main node and the remote node.

```toml
[internal_grpc]
enabled = true
listen = "0.0.0.0:50051"

[internal_grpc.security]
mode = "mtls" # mtls | tls | disabled
cert_dir = "~/.agenthub/internal-grpc"

[internal_grpc.auth]
shared_secret = "replace-me"
issuer = "agenthub"
audience = "agenthub-internal"
```

Then register the remote node from the `Agents` page or `agent_nodes` API with:

- `id`
- `grpc_target`
- `tls_server_name`
- `default_worktree_root` (optional)

See [docs/features/agent-nodes.md](docs/features/agent-nodes.md) for the
current contract and rollout model.

## Architecture At A Glance

- **AgentHub server**
  - Rust backend serving the API, embedded web UI, and runtime control plane
- **ACP runtime**
  - Structured event model for plans, tools, output, and history replay
- **Web workbench**
  - Mantine primitives plus Tailwind utilities, shared UI primitives, and
    bounded recent-window conversation views for ACP and Team surfaces
- **Teams runtime**
  - Leader/worker orchestration, Team tasks, mailbox coordination, and shared
    conversation flow
- **Actor CLI**
  - Canonical runtime coordination interface for actor mailbox and Team control
    actions
- **Agent Nodes**
  - Optional internal gRPC control and relay path for remote execution
- **Persistence**
  - SQLite for sessions, agent config, audit records, and Team state

## Documentation Map

- Published user docs: [doc.agenthub.hawkingrei.com](https://doc.agenthub.hawkingrei.com/)
  - Start with Product Overview, Feature Overview, and Architecture Overview
- Local user docs source: [userdocs/](userdocs/)
- Internal docs guide: [docs/README.md](docs/README.md)
- Project charter and engineering constraints: [AGENTS.md](AGENTS.md)
- Agent and Team architecture: [docs/features/agents-teams.md](docs/features/agents-teams.md)
- Frontend/UI architecture: [docs/features/frontend-design.md](docs/features/frontend-design.md)
- Actor runtime and mailbox model: [docs/features/actor-foundation.md](docs/features/actor-foundation.md)
- ACP runtime contract: [docs/features/acp-runtime.md](docs/features/acp-runtime.md)
- Remote execution nodes: [docs/features/agent-nodes.md](docs/features/agent-nodes.md)
- Distributed node architecture: [docs/features/distributed-node-architecture.md](docs/features/distributed-node-architecture.md)
- Team workbench user guide: [userdocs/docs/advanced/team-workbench.md](userdocs/docs/advanced/team-workbench.md)
- Active follow-up backlog: [docs/todo.md](docs/todo.md)
- API payload naming rules: [docs/api_naming.md](docs/api_naming.md)

For user-facing documentation preview/build:

```bash
npm --prefix userdocs ci
npm --prefix userdocs run start
npm --prefix userdocs run build
```

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

## Development

### Common Commands

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
bazel coverage --combined_report=lcov --test_output=errors //crates/agenthub-text:agenthub_text_tests # Example for a single crate
```

### Recommended Pre-PR Checks

```bash
# Rust + proto guard
cargo test
make proto-check

# Web checks
npm --prefix web run lint
npm --prefix web run test:coverage

# User docs
npm --prefix userdocs run build

# Optional: E2E smoke
npm --prefix web run e2e -- tests/e2e/app.e2e.ts --project=chromium
```

### CI Pipelines

- `Rust`: cargo check + coverage (`rust-cargo.lcov`) + Codecov upload
- `Clippy`: `cargo clippy --workspace --all-targets -- -D warnings`
- `Web`: lint + unit coverage + build + Codecov upload
- `Web E2E`: Playwright coverage + Codecov upload
- `Bazel`: split `Bazel Build`, `Bazel Test (Root)`, `Bazel Test (Crates)`, and `Bazel Coverage`
  jobs, with `bazel.lcov` uploaded to Codecov and an aggregate `Bazel Build and Test` gate
- `User Docs`: Docusaurus build validation

## License

Apache-2.0
