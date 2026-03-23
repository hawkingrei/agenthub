---
sidebar_position: 2
---

# Configuration Basics

This page lists the minimum settings needed for normal day-to-day AgentHub
usage.

## Config File

AgentHub reads configuration from `~/.agenthub/config.toml` by default.

Start from a minimal single-node baseline:

```toml
[server]
listen = "127.0.0.1:8080"

safe_paths = [
  "/home/you/projects",
  "/home/you/sandboxes",
]

[worktree]
default_root = "/home/you/.agenthub/worktrees"

[history]
event_retention_days = 5
vacuum_on_cleanup = false
```

## Required Fields For Daily Use

- `server.listen`: where the UI and API are served
- `safe_paths`: allowed workdir roots for agent runs
- `worktree.default_root`: default base for `create_worktree` mode

If these are missing or incorrect, users usually see login, start, or path
validation errors.

## Notes About `safe_paths`

- Keep `safe_paths` as short and explicit as possible.
- Prefer repository roots over broad paths such as `/` or your full home
  directory.
- AgentHub always keeps the default worktree root reachable so
  `create_worktree` mode can derive safe execution paths cleanly.

## Optional Remote-Node Settings

If you plan to use remote Agent Nodes, add an `internal_grpc` block as well:

```toml
[internal_grpc]
enabled = true
listen = "127.0.0.1:50051"

[internal_grpc.security]
mode = "tls"
cert_dir = "~/.agenthub/internal-grpc"

[internal_grpc.auth]
issuer = "agenthub"
audience = "agenthub-internal"
shared_secret = "replace-me"
```

This is optional for single-node deployments. It becomes required once the main
control plane must register and control remote-target agents.

## First Validation After Config Update

1. Restart AgentHub.
2. Log in through the browser.
3. Create one test agent in `create_worktree` mode.
4. Confirm the generated path is under `worktree.default_root`.
5. Confirm a path outside `safe_paths` is rejected.
