# AgentHub

[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](LICENSE)
[![User Docs](https://github.com/hawkingrei/agenthub/actions/workflows/userdocs.yml/badge.svg)](https://github.com/hawkingrei/agenthub/actions/workflows/userdocs.yml)
[![GitHub release](https://img.shields.io/github/v/release/hawkingrei/agenthub?label=release)](https://github.com/hawkingrei/agenthub/releases)
[![Docs](https://img.shields.io/badge/docs-live-2ea44f.svg)](https://doc.agenthub.hawkingrei.com/)

AgentHub is a self-hosted AI agent control plane for long-lived coding agents,
structured ACP timelines, multi-agent Team workflows, and optional remote
execution nodes.

It is designed for teams that want one product surface for long-running coding
agents, structured ACP review, shared Team coordination, and remote execution
without introducing a separate control plane.

Quick links: [Docs Site](https://doc.agenthub.hawkingrei.com/) ·
[Install AgentHub](#install-agenthub) ·
[Why AgentHub](#why-agenthub) ·
[Remote Agent Nodes](#remote-agent-nodes) ·
[Documentation](#documentation) ·
[Developer Docs](docs/README.md)

## Why AgentHub

Most agent tools are optimized for a single terminal session. AgentHub is built
for the operational side of agent workflows.

It keeps long-running agents, structured ACP review, Team coordination, remote
agent nodes, and persistent runtime state in one product surface instead of
splitting them across disposable chats, terminal tabs, and ad hoc scripts.

In practice, AgentHub sits closer to an AI agent control plane or shared
workbench than to a thin prompt UI.

## What You Can Do

- Create, start, stop, reconnect, and delete agents
- View ACP events such as messages, plans, tool calls, command output, and
  debug streams
- Run multi-agent Team workflows with coordinator/worker coordination
- Install AgentHub as an app-like PWA shell while still picking up fresh web
  deploys on refresh
- Register remote execution nodes and start agents on those nodes
- Persist session history and operational records in SQLite
- Receive completion notifications in the web UI

## Install AgentHub

Official release binaries are available for:

- macOS Apple Silicon (`darwin-arm64`)
- Linux `x86_64`
- Linux `aarch64`

Windows and macOS Intel binaries are not currently published.

Recommended installation paths:

- Ubuntu/Debian: install the matching `.deb` from the
  [latest release](https://github.com/hawkingrei/agenthub/releases/latest). It
  includes `agenthub`, `agenthubd`, and a systemd service.
- macOS or portable Linux: install the matching `agenthub` and `agenthubd`
  archives from the same release and verify them with `SHA256SUMS.txt`.
- npm: `npm install -g @linkerdog/agenthub` installs both native files through
  the matching platform package.

The Homebrew tap currently trails the primary release channel and installs a
legacy ACP helper. Use release archives for a new complete installation until
the formula is brought back into version and adapter parity.

After installing both binaries:

```bash
agenthub --version
agenthubd --version
agenthub init
agenthubd
```

Then open `http://localhost:8080`. For package-specific commands, checksum
verification, upgrades, uninstall behavior, platform limitations, and the
current Linux runtime caveat, see
[Installation and Startup](https://doc.agenthub.hawkingrei.com/docs/getting-started/installation).

For local source development, contributor setup, repository layout, and common
commands, see [docs/developer-setup.md](docs/developer-setup.md).

## Product Overview

AgentHub is a Rust-based control plane for operating AI agents beyond one
ephemeral terminal tab.

It combines:

- a single Rust backend
- an embedded React web UI
- ACP-based structured output rendering
- SQLite-backed persistence
- Team coordinator/worker orchestration
- optional remote execution over internal gRPC

The main design goal is simple: keep AI agents observable, controllable, and
recoverable even when the browser closes, the task lasts for hours, or the
work must be split across multiple agents and machines.

AgentHub is built for engineering teams that want a practical AI agent
workspace instead of a disposable chat box.

## Highlights

- `⏳` **Long-lived agent control**
  - Keep coding agents alive across browser refreshes and disconnects
- `🧾` **Structured ACP timelines**
  - Inspect plans, tool calls, command output, and replayable history
- `👥` **Team workflows**
  - Coordinate coordinator/worker execution with channels, Kanban, and ACP views
- `🌐` **Remote agent nodes**
  - Run agents on other machines while keeping one main control plane
- `💾` **Persistent runtime state**
  - Store session history, operational state, and audit records in SQLite

In one product surface, you can:

- create, start, stop, reconnect, and delete agents
- keep sessions alive after the browser tab closes
- restart or recover stuck sessions without losing the operational surface
- review structured agent output and replayable history
- inspect per-agent execution details when runtime debugging is needed
- run multi-agent Team workflows in a shared workspace
- route execution to remote nodes when one machine is not enough

### Multi-Agent Team Workflows

AgentHub includes a Team workbench for coordinator/worker coordination.

Core concepts:

- `Channels` for shared coordination
- `# all` as the default Team lane
- `Kanban` as the canonical Team task surface
- per-member ACP inspection when deep runtime debugging is needed

## Agent Team Highlights

- `👥` **Multi-agent collaboration**
  - Organize multiple AI agents in one Team instead of one isolated session.
- `🧩` **Shared workspace**
  - Keep people and agents in one workspace with shared context and progress.
- `💬` **Channel-based communication**
  - Talk to the whole Team in shared channels instead of scattered side sessions.
- `🧵` **Threaded follow-up**
  - Reply in thread for a specific question or update without derailing the main channel.
- `📋` **Task coordination**
  - Manage planning, ownership, and status transitions in a dedicated task surface.
- `👀` **Visible progress**
  - See what each agent is doing, what changed, and where work is blocked.
- `🔍` **Per-agent inspection**
  - Open one member and inspect its ACP timeline and execution details directly.
- `⏱️` **Built for long-running work**
  - Let Team work continue beyond one browser session or one short interactive turn.
- `🛠️` **Role-based division of work**
  - Split planning, implementation, review, and verification across different agents.
- `🎛️` **Unified control surface**
  - Start, stop, inspect, and steer the whole Team from one product interface.
- `🌐` **Remote execution ready**
  - Run Team members on different machines while keeping one shared control plane.

## Remote Agent Nodes

If you want remote execution, run the same `agenthub` release on every machine.
Keep the main process in its default `main` role, enable internal gRPC on both
sides, and configure each remote process with `role = "node"` plus a unique
`node_id`.

```toml
[server]
role = "node"
node_id = "node-east"

[internal_grpc]
enabled = true
listen = "0.0.0.0:50051"

[internal_grpc.auth]
shared_secret = "<shared-secret-from-your-secret-store>"
issuer = "agenthub"
audience = "agenthub-internal"

[internal_grpc.bootstrap]
token = "<node-bootstrap-token>"
```

Use TLS or mTLS on a private network, then register the reachable gRPC target
from the root-only node controls on the **Agents** page. See the
[Agent Nodes user guide](https://doc.agenthub.hawkingrei.com/docs/core/agent-nodes)
for onboarding and the current transport boundary.

## Documentation

### For Users

- Docs site: [doc.agenthub.hawkingrei.com](https://doc.agenthub.hawkingrei.com/)
- Product overview: [Product Overview](https://doc.agenthub.hawkingrei.com/docs/overview/product-overview)
- Feature overview: [Feature Overview](https://doc.agenthub.hawkingrei.com/docs/overview/feature-overview)
- Installation: [Installation and Startup](https://doc.agenthub.hawkingrei.com/docs/getting-started/installation)
- Team workbench guide: [Team Workbench](https://doc.agenthub.hawkingrei.com/docs/advanced/team-workbench)
- Agent nodes: [Agent Nodes and Remote Execution](https://doc.agenthub.hawkingrei.com/docs/core/agent-nodes)

### For Developers

- Developer docs index: [docs/README.md](docs/README.md)
- Developer setup: [docs/developer-setup.md](docs/developer-setup.md)
- Architecture map: [docs/architecture-map.md](docs/architecture-map.md)
- Active engineering backlog: [docs/todo.md](docs/todo.md)
- Project charter and constraints: [AGENTS.md](AGENTS.md)

## Who AgentHub Is For

AgentHub is a strong fit for:

- engineers running long-lived coding agents
- teams experimenting with coordinator/worker multi-agent workflows
- operators who need structured runtime visibility
- organizations that want self-hosted agent control instead of opaque hosted sessions
- users who need one control plane across local and remote execution targets

## Development

Source development workflow, repository layout, common commands, and CI
expectations live in [docs/developer-setup.md](docs/developer-setup.md).

## License

Apache-2.0

## Star History

<a href="https://www.star-history.com/?repos=hawkingrei%2Fagenthub&type=date&legend=top-left">
 <picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/chart?repos=hawkingrei/agenthub&type=date&theme=dark&legend=top-left" />
   <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/chart?repos=hawkingrei/agenthub&type=date&legend=top-left" />
   <img alt="Star History Chart" src="https://api.star-history.com/chart?repos=hawkingrei/agenthub&type=date&legend=top-left" />
 </picture>
</a>
