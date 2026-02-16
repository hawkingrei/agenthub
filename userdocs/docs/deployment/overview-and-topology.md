---
sidebar_position: 1
---

# Deployment Overview and Topology

This page describes a practical deployment model for running AgentHub as a
single service.

## Architecture at a Glance

AgentHub deployment is intentionally simple:

- One backend process (Rust)
- One embedded web UI served by that same process
- One SQLite database for persisted state
- Browser clients connected with HTTP + SSE

## Runtime Components

Prepare these components before rollout:

1. AgentHub executable (or `cargo run` for development)
2. A valid `config.toml`
3. Writable runtime home (default under `~/.agenthub/`)
4. Explicit `safe_paths` for all allowed repositories/workdirs

## Deployment Modes

### Local development mode

Use this for daily personal work:

```bash
cd web
npm install
npm run build
cd ..
cargo run -- -c /path/to/config.toml
```

### Team internal mode

Use this for shared environments:

- Build and run a fixed binary
- Run under a dedicated OS user (not root)
- Manage process lifecycle with a supervisor (for example systemd)
- Keep AgentHub behind an internal reverse proxy or VPN boundary

## Recommended Network Shape

- Reverse proxy terminates TLS and forwards to AgentHub
- AgentHub listens on internal/private address where possible
- Users access a single stable URL (for login, UI, and API)

## Basic Startup Commands

If you build a release binary:

```bash
./agenthub -c /path/to/config.toml
```

If you run from source:

```bash
cargo run -- -c /path/to/config.toml
```

## Post-Deploy Smoke Checklist

1. Open AgentHub UI and verify login works.
2. Create one agent with a safe test path.
3. Start a short task and confirm status reaches a terminal state.
4. Refresh browser and verify session history still exists.
5. Confirm a path outside `safe_paths` is rejected.

## Related Pages

- [Production Checklist](./production-checklist.md)
- [Troubleshooting](../operations/troubleshooting.md)
- [Configuration Basics](../getting-started/configuration-basics.md)
