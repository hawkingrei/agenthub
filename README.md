# AgentHub

AgentHub is a single-binary service for managing and interacting with remote AI agents. It provides a web UI, ACP-based structured output rendering, and persistent sessions backed by SQLite. It also supports A2A (agent-to-agent) orchestration for multi-agent workflows.

## Features

- Create, start, stop, and delete agents
- ACP event rendering (messages, tool calls, plan, commands, debug)
- A2A orchestration for multi-agent collaboration
- Session persistence even when the browser is closed
- Passkey-based login with device join flow
- Safe path enforcement and audit logging
- Web Push notifications (VAPID management UI)
- Embedded frontend (Vite build served by Rust)

## Requirements

- Rust (stable)
- Node.js 20+ (for the web build)
- Bazel / Bazelisk (optional, for Bazel-driven Rust build + test entrypoints)

## Quick Start

```bash
# Build the web UI
cd web
npm install
npm run build

# Run the server
cd ..
cargo run
```

The server serves the UI on `http://localhost:8080`.

## Configuration

AgentHub reads configuration from a `config.toml` file. Example:

```toml
listen_addr = "0.0.0.0:8080"

safe_paths = [
  "/home/foo",
  "/home/foo/projects"
]
```

The database and runtime state live under `~/.agenthub/` by default.

## Development

```bash
# Rust tests
cargo test

# Web tests
cd web
npm run test

# Bazel-driven checks
bazel build //...
bazel test //...
```

## Project Layout

```
agenthub/
  src/                # Rust server
  web/                # Vite + React frontend
  agenthub-codex-acp/ # ACP adapter (workspace member)
  migrations/         # DB migrations
  AGENTS.md
```

## License

MIT
