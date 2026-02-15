---
sidebar_position: 2
---

# Installation and Startup

## Prerequisites

- Rust (stable toolchain)
- Node.js 20+

## Start AgentHub Locally

```bash
# Build frontend assets
cd web
npm install
npm run build

# Start backend service
cd ..
cargo run
```

By default, AgentHub serves the UI at `http://localhost:8080`.

## Runtime Data Location

AgentHub stores data under `~/.agenthub/` by default, including:

- SQLite database
- Runtime session state
- Logs and operational files

## Configuration

AgentHub reads settings from `config.toml`.

Example:

```toml
listen_addr = "0.0.0.0:8080"

safe_paths = [
  "/home/you",
  "/home/you/projects"
]
```

Set `safe_paths` to the directories users are allowed to use as agent workdirs.
