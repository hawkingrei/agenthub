---
sidebar_position: 1
---

# Installation and Startup

## Prerequisites

- Rust (stable toolchain)
- Node.js 20+
- Git

## Build The Web UI

```bash
npm --prefix web ci
npm --prefix web run build
```

## Create A Minimal Config

AgentHub reads configuration from `~/.agenthub/config.toml` by default.

Start with a minimal single-node config:

```toml
[server]
listen = "127.0.0.1:8080"

safe_paths = [
  "/home/you/projects",
  "/home/you/sandboxes",
]

[worktree]
default_root = "/home/you/.agenthub/worktrees"
```

`safe_paths` should list the repository roots that users are allowed to use as
agent workdirs.

## Start AgentHub

For the standard local workflow:

```bash
make run
```

If you want to point at an explicit config file:

```bash
cargo run -- -c /path/to/config.toml
```

By default, AgentHub serves the UI at `http://localhost:8080`.

## Runtime Data Location

AgentHub stores runtime data under `~/.agenthub/` by default, including:

- SQLite database
- runtime session state
- logs and operational files

## Smoke Check

After startup:

1. Open `http://localhost:8080`
2. Confirm the login page loads
3. Create one test agent
4. Start it and send one simple instruction
5. Confirm output appears in `Conversation`
