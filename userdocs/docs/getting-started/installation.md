---
sidebar_position: 1
---

# Installation and Startup

## Install Release Binaries With Homebrew

AgentHub publishes Homebrew release binaries through the `linkerdog/homebrew-tap`
tap.

```bash
brew tap linkerdog/homebrew-tap
brew install linkerdog/homebrew-tap/agenthub
```

This installs:

- `agenthub`
- `agenthub-acp`

To run AgentHub as a background service:

```bash
brew services start linkerdog/homebrew-tap/agenthub
```

AgentHub reads configuration from `~/.agenthub/config.toml`.

To create that file interactively:

```bash
agenthub init
```

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

## Install Debian Packages

Linux releases also publish Debian packages for Ubuntu/Debian-style hosts:

- `agenthub_<version>_amd64.deb`
- `agenthub_<version>_arm64.deb`

Download the package that matches your host and install it with `apt`:

```bash
sudo apt install ./agenthub_<version>_amd64.deb
```

The Debian package installs:

- `agenthub`
- `agenthub-acp`
- `agenthub.service`

On install, the package creates an `agenthub` system user, creates runtime data under
`/var/lib/agenthub`, enables the service, and attempts to start it.

Check service status:

```bash
systemctl status agenthub.service
journalctl -u agenthub.service -f
```

The service reads configuration from:

```text
/var/lib/agenthub/.agenthub/config.toml
```

The default package config listens on `127.0.0.1:8080` and allows workspaces under
`/var/lib/agenthub/workspaces`. Edit the config and restart the service when you need a different
listen address or repository root:

```bash
sudo systemctl restart agenthub.service
```

## Build From Source

### Prerequisites

- Rust `1.96.0` (stable toolchain baseline)
- Node.js 20+
- Git

### Install Web Dependencies

```bash
npm --prefix web ci
```

### Create A Minimal Config

AgentHub reads configuration from `~/.agenthub/config.toml` by default.

If you want a guided first-run setup instead of hand-writing TOML, run:

```bash
cargo run -- init
```

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

### Start AgentHub

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

### ACP Provider Baseline

The canonical AgentHub ACP adapter binary is `agenthub-acp`. Use `agenthub-acp codex` for Codex
sessions and `agenthub-acp claude` for Claude sessions. Existing deployments that still launch
`agenthub-codex-acp` should migrate to `agenthub-acp codex` before relying on current release
packages.

- Current repository baseline: official Codex `0.138.x`
- If you override `codex_acp.binary`, keep the replacement ACP binary protocol-
  compatible with the same Codex generation

### Optional App Install

On supported browsers, AgentHub can be installed as a standalone web app.

- The frontend registers its service worker automatically.
- The same service worker is used for push notifications and installability.
- AgentHub is **not** offline-first: a normal refresh still fetches the latest
  app shell and hashed assets from the server.

For non-localhost deployments, use HTTPS if you want installability and browser
push to work reliably.

### Runtime Data Location

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

### Smoke Check

After startup:

1. Open `http://localhost:8080`
2. Confirm the login page loads
3. Create one test agent
4. Start it and send one simple instruction
5. Confirm output appears in `Conversation`
