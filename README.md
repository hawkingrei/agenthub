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
[Why Teams And ACP Matter](#why-teams-and-acp-matter) ·
[Remote Agent Nodes](#remote-agent-nodes) ·
[Documentation](#documentation) ·
[Developer Docs](docs/README.md)

## Install AgentHub
## Why AgentHub

Most agent tools are optimized for a single terminal session. AgentHub is built
for the operational side of agent workflows:

- Keep agent sessions alive after the browser tab closes
- Inspect structured ACP timelines instead of raw terminal scrollback
- Run coordinator/worker Team workflows with shared coordination primitives
- Route execution to remote Agent Nodes over internal gRPC when one machine is
  not enough
- Keep audit history, runtime state, and operator control in one place

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

### Homebrew

AgentHub publishes Homebrew release binaries through the
`linkerdog/homebrew-tap` tap.

```bash
brew tap linkerdog/homebrew-tap
brew install linkerdog/homebrew-tap/agenthub
```

Installed binaries:

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

For local source development, contributor setup, repository layout, and common
commands, see [docs/developer-setup.md](docs/developer-setup.md).

## Why AgentHub

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
  - Keep coding agents alive across browser refreshes and disconnects.
- `🧾` **Structured ACP timelines**
  - Inspect plans, tool calls, command output, and replayable history.
- `👥` **Team workflows**
  - Coordinate coordinator/worker execution with channels, Kanban, and ACP views.
- `🌐` **Remote agent nodes**
  - Run agents on other machines while keeping one main control plane.
- `💾` **Persistent runtime state**
  - Store session history, operational state, and audit records in SQLite.

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

## Why Teams And ACP Matter

Many agent tools optimize for one local chat or terminal session. AgentHub
optimizes for operational continuity.

AgentHub is useful when you need:

- long-lived AI coding agents
- structured output instead of terminal-only logs
- multi-agent Team orchestration
- recoverable runtime state
- remote agent execution
- operator-visible audit history

In practice, that means AgentHub sits closer to a control plane or workbench
than to a thin prompt UI.

## Remote Agent Nodes

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

Then register the remote node from the `Agents` page or the `agent_nodes` API
with:

- `id`
- `grpc_target`
- `tls_server_name`
- `default_worktree_root` (optional)

For the current contract, see
[docs/features/agent-nodes.md](docs/features/agent-nodes.md).

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
