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
safe_paths = [
  "/home/you/projects",
  "/home/you/sandboxes",
]

[server]
listen = "127.0.0.1:8080"

[worktree]
default_root = "/home/you/.agenthub/worktrees"

[history]
event_retention_days = 5
vacuum_on_cleanup = false
```

## Required Fields For Daily Use

- `server.listen`: where the UI and API are served
- `safe_paths`: top-level allowlist of workdir roots for agent runs
- `worktree.default_root`: default base for `create_worktree` mode

If these are missing or incorrect, users usually see login, start, or path
validation errors.

## Notes About `safe_paths`

- Keep `safe_paths` as short and explicit as possible.
- Prefer repository roots over broad paths such as `/` or your full home
  directory.
- By default, `~/.agenthub/worktrees` is automatically included in the
  effective safe-path allowlist, so the default `create_worktree` layout works
  without extra configuration.
- If you change `worktree.default_root` to another location, add that root or a
  containing directory to `safe_paths` as well, or `create_worktree` path
  validation will fail.

## Internal gRPC Settings

Add an `internal_grpc` block when either of these is true:

- you plan to register or control remote Agent Nodes
- you want `agenthub actor ...` commands such as `team-members`, `inbox`,
  `ack`, `send`, or `time-trigger-*` to talk to the authority control plane

Recommended single-node baseline:

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
shared_secret = "replace-this-with-a-long-random-secret"

[internal_grpc.bootstrap]
# optional unless you bootstrap remote nodes
token = "replace-me-if-you-use-node-bootstrap"
```

Why the explicit `shared_secret` matters:

- the authority server can auto-generate and persist a secret under
  `cert_dir/auth_secret.txt`
- `agenthub actor ...` is a client, not a server; it only reads
  `~/.agenthub/config.toml` when minting its loopback token
- if `shared_secret` is omitted from the config file, the actor CLI cannot mint
  a valid local internal token even when the authority server already started

Operational notes:

- `agenthub actor ...` does not start the internal gRPC server for you. A
  separate AgentHub authority process must already be running with
  `internal_grpc.enabled = true`.
- `tls` is the recommended default. Use `mtls` when you need client-certificate
  validation between nodes. `disabled` is for local development/testing only.
- In a normal shell, mailbox commands usually need an explicit `--actor-id`.
  Inside an injected actor runtime shell, `AGENTHUB_ACTOR_ID` and
  `AGENTHUB_ACTOR_CURRENT_RUN_ID` may provide the fallback scope instead.

## First Validation After Config Update

1. Restart AgentHub.
2. Confirm the server is listening on the configured internal gRPC address.
3. If you use `agenthub actor ...` outside an injected runtime shell, include
   `--actor-id <actor_id>` explicitly.
4. Run one authority-side read command such as `agenthub actor team-members
   --actor-id <actor_id> --run-id <run_id>`.
5. Log in through the browser.
6. Create one test agent in `create_worktree` mode.
7. Confirm the generated path is under `worktree.default_root`.
8. Confirm a path outside `safe_paths` is rejected.
