---
sidebar_position: 3
---

# Configuration Basics

This page lists the minimum settings needed for daily user operation.

## Config File

AgentHub reads configuration from `config.toml`.

Start from a minimal baseline:

```toml
listen_addr = "0.0.0.0:8080"

safe_paths = [
  "/home/you",
  "/home/you/projects"
]

[worktree]
default_root = "/home/you/.agenthub/worktrees"
```

## Required Fields for Users

- `listen_addr`: where UI/API are served
- `safe_paths`: allowed workdir roots for agent runs
- `worktree.default_root`: default base for `create_worktree` mode

If these are missing or incorrect, users will usually see login/start/path
errors.

## Safe Paths Guidance

- Keep `safe_paths` as short as possible
- Prefer explicit project roots over broad system paths
- Avoid adding `/` or home root as a blanket allow rule

## First Validation After Config Update

1. Restart AgentHub server
2. Login in browser
3. Create one test agent in `create_worktree` mode
4. Confirm generated path is under `worktree.default_root`
5. Confirm a path outside `safe_paths` is rejected
