---
sidebar_position: 1
---

# Installation and Startup

## Prerequisites

- Rust (stable toolchain)
- Node.js 20+
- Git

## Install Web Dependencies

```bash
npm --prefix web ci
```

## Create A Minimal Config

AgentHub reads configuration from `~/.agenthub/config.toml` by default.

Start with a minimal single-node config:

```toml
safe_paths = [
  "/home/you/projects",
  "/home/you/sandboxes",
]

[server]
listen = "127.0.0.1:8080"

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

`make run` builds the web UI as part of the normal startup path, so you do not
need a separate `npm --prefix web run build` step for the default local setup.

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

For Codex-backed ACP sessions, AgentHub also materializes its managed Team and
runtime skills under `~/.agents/skills/agenthub-runtime/...`.

- No manual `~/.codex/config.toml` skill configuration is required.
- Global managed skills should be referenced through an absolute path or
  `~/...`; do not treat them as current-workdir-relative `.agents/...` paths.
- Repo-local `.agents/skills` continue to work independently alongside the
  managed AgentHub skill set.

## Smoke Check

After startup:

1. Open `http://localhost:8080`
2. Confirm the login page loads
3. Create one test agent
4. Start it and send one simple instruction
5. Confirm output appears in `Conversation`
