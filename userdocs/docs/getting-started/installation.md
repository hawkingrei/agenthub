---
sidebar_position: 1
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

## Run With Explicit Config

If your setup uses a custom config path, run:

```bash
cargo run -- -c /path/to/config.toml
```

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

## Smoke Check

After startup:

1. Open `http://localhost:8080`
2. Confirm login page loads
3. Create one test agent
4. Start and send one simple instruction
5. Confirm output appears in Conversation
