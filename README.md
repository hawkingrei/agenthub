# AgentHub

AgentHub is a single-binary control plane for long-lived AI agents.
It combines a Rust backend, an embedded React web UI, ACP-based structured
output rendering, SQLite-backed persistence, multi-agent Team orchestration,
and optional remote execution nodes.

Quick links: [Docs Site](https://doc.agenthub.hawkingrei.com/) ·
[Why AgentHub](#why-agenthub) · [Install](#install) · [Developer Guide](docs/developer-setup.md) ·
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

## Install

### Homebrew

AgentHub publishes Homebrew release binaries through the `linkerdog/homebrew-tap`
tap.

```bash
brew tap linkerdog/homebrew-tap
brew install linkerdog/homebrew-tap/agenthub
```

This installs:

- `agenthub`
- `agenthub-codex-acp`

To run AgentHub in the background:

```bash
brew services start linkerdog/homebrew-tap/agenthub
```

AgentHub reads config from `~/.agenthub/config.toml`.

Minimal example:

```toml
[server]
listen = "127.0.0.1:8080"
```

Then open `http://localhost:8080`.

Current release binaries are available for:

- macOS Apple Silicon (`darwin-arm64`)
- Linux `x86_64`
- Linux `aarch64`

### Build From Source

For local source development, setup, common commands, and CI expectations, see
[docs/developer-setup.md](docs/developer-setup.md).

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
- Developer setup and workflow: [docs/developer-setup.md](docs/developer-setup.md)
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

## Development

Development workflow, repository layout, common commands, and CI expectations
live in [docs/developer-setup.md](docs/developer-setup.md).

## License

Apache-2.0
