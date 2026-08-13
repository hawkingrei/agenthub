---
sidebar_position: 1
---

# Installation and Startup

AgentHub publishes native binaries for these platforms:

| Platform | Release target | Debian package | npm package |
| --- | --- | --- | --- |
| macOS Apple Silicon | `darwin-arm64` | No | Yes |
| Linux x86_64 | `linux-amd64` | `amd64` | Yes |
| Linux ARM64 | `linux-arm64` | `arm64` | Yes |

Windows and macOS Intel binaries are not currently published.

For a complete agent runtime, install both executables from the same release:

- `agenthub`: the control plane and embedded web UI
- `agenthub-acp`: the provider adapter used by the default Codex and Claude
  runtime commands

The Debian package contains both executables. GitHub publishes them as separate
archives. The npm wrapper currently contains only `agenthub`.

:::caution Linux runtime baseline

The minimum supported glibc version for official Linux artifacts is not frozen
yet. A recent `main` prebuild was verified on Ubuntu 24.04 and required
`GLIBC_2.38` / `GLIBC_2.39`; it did not start on Ubuntu 22.04. Validate the
binary on the target host before production rollout and check the selected
release notes for release-specific compatibility evidence.

:::

## Choose an Installation Method

| Method | Best for | Includes `agenthub-acp` | Service integration |
| --- | --- | --- | --- |
| Debian package | Ubuntu/Debian servers | Yes | systemd unit included |
| GitHub archives | macOS and portable Linux installs | Install the matching archive | Manual |
| npm | Existing Node.js environments | No | Manual |
| Homebrew tap | Legacy installations only | Legacy helper only | Homebrew service |

## Debian Package

Use the Debian package for a system-managed Linux installation. Install the
GitHub CLI first, or download the matching `.deb` and `SHA256SUMS.txt` from the
[latest release](https://github.com/hawkingrei/agenthub/releases/latest).

For Linux x86_64:

```bash
mkdir -p agenthub-install
cd agenthub-install
gh release download --repo hawkingrei/agenthub \
  --pattern 'agenthub_*_amd64.deb' \
  --pattern SHA256SUMS.txt
sha256sum --ignore-missing -c SHA256SUMS.txt
sudo apt install ./agenthub_*_amd64.deb
```

For Linux ARM64, replace `amd64` with `arm64` in the download pattern and
package name.

The package installs:

- `/usr/bin/agenthub`
- `/usr/bin/agenthub-acp`
- `agenthub.service`

It also creates an `agenthub` system user, enables the service, and attempts to
start it. Runtime data is stored under `/var/lib/agenthub`, and the generated
configuration is:

```text
/var/lib/agenthub/.agenthub/config.toml
```

Check the installation:

```bash
agenthub --version
agenthub-acp --version
sudo systemctl status agenthub.service --no-pager
sudo journalctl -u agenthub.service -n 100 --no-pager
```

The package config listens on `127.0.0.1:8080` and allows workspaces under
`/var/lib/agenthub/workspaces`. For a remote server, either access it through
an SSH tunnel:

```bash
ssh -L 8080:127.0.0.1:8080 user@agenthub-host
```

or place AgentHub behind an authenticated HTTPS reverse proxy. Do not expose an
unconfigured instance directly to a public network.

Edit the service configuration with:

```bash
sudoedit /var/lib/agenthub/.agenthub/config.toml
sudo systemctl restart agenthub.service
```

## GitHub Release Archives

Use release archives for macOS or a user-managed Linux installation. This
example downloads the latest matching `agenthub` and `agenthub-acp` archives.

Set `TARGET` to one of:

- `darwin-arm64`
- `linux-amd64`
- `linux-arm64`

```bash
TARGET=darwin-arm64
mkdir -p agenthub-install
cd agenthub-install
gh release download --repo hawkingrei/agenthub \
  --pattern "agenthub-[0-9]*-${TARGET}.tar.gz" \
  --pattern "agenthub-acp-*-${TARGET}.tar.gz" \
  --pattern SHA256SUMS.txt
```

Verify the downloaded assets. On macOS:

```bash
grep -- "-${TARGET}.tar.gz$" SHA256SUMS.txt | shasum -a 256 -c -
```

On Linux:

```bash
grep -- "-${TARGET}.tar.gz$" SHA256SUMS.txt | sha256sum -c -
```

Extract and install both binaries into a user-owned directory:

```bash
tar -xzf agenthub-[0-9]*-${TARGET}.tar.gz
tar -xzf agenthub-acp-*-${TARGET}.tar.gz
mkdir -p "$HOME/.local/bin"
install -m 0755 agenthub-[0-9]*-${TARGET}/agenthub "$HOME/.local/bin/agenthub"
install -m 0755 agenthub-acp-*-${TARGET}/agenthub-acp "$HOME/.local/bin/agenthub-acp"
export PATH="$HOME/.local/bin:$PATH"
```

Persist the `PATH` update in your shell profile, then verify both commands:

```bash
agenthub --version
agenthub-acp --version
```

Keep the two archives on the same release tag. Mixing the control plane with a
different ACP adapter generation can cause provider startup or protocol errors.

## npm

The npm package requires Node.js 18 or newer and supports macOS Apple Silicon,
Linux x86_64, and Linux ARM64:

```bash
npm install -g @linkerdog/agenthub
agenthub --version
```

The npm wrapper installs only the native `agenthub` control-plane binary. To
run the default Codex or Claude agent commands, also install the matching
`agenthub-acp` archive from the same GitHub release by following the archive
instructions above.

If npm reports that the platform package is missing, confirm that optional
dependencies are enabled and reinstall:

```bash
npm config get optional
npm install -g @linkerdog/agenthub@latest --include=optional
```

## Homebrew Channel Status

:::warning

The `linkerdog/homebrew-tap` formula currently trails the primary GitHub and npm
release channels and installs the legacy `agenthub-codex-acp` helper instead of
the current `agenthub-acp` adapter. Do not use it for a new installation that
needs the current complete runtime. Use the GitHub archives instead.

:::

Existing Homebrew users can inspect the pinned formula and installed binaries
before deciding whether to migrate:

```bash
brew update
brew info linkerdog/homebrew-tap/agenthub
agenthub --version
command -v agenthub-acp
command -v agenthub-codex-acp
```

## First Startup

Archive and npm installations read configuration from
`~/.agenthub/config.toml`. Create it with the interactive initializer:

```bash
agenthub init
```

`agenthub init` requires an interactive terminal. It configures the instance
role and internal gRPC bootstrap, but it does not configure provider API keys
or provider-specific base URLs.

For a minimal local instance, the resulting configuration should include:

```toml
safe_paths = [
  "/home/you/projects",
]

[server]
listen = "127.0.0.1:8080"

[worktree]
default_root = "/home/you/.agenthub/worktrees"
```

On macOS, replace the Linux paths with paths under your home directory.

Start AgentHub in the foreground:

```bash
agenthub
```

Then open [http://localhost:8080](http://localhost:8080), create the first
account, and continue with the [first task walkthrough](./first-task-walkthrough.md).

Before starting an agent, configure any provider credentials required by the
selected adapter. Keep secrets in the provider-supported environment or
credential store; do not commit them to the AgentHub config or a workspace.

## Upgrade

### Debian package

Download and verify the new package, then install it over the existing version:

```bash
sudo apt install ./agenthub_<new-version>_amd64.deb
sudo systemctl status agenthub.service --no-pager
```

The package preserves `/var/lib/agenthub` during upgrades.

### GitHub archives

Stop the foreground process or your service manager, download both archives
from the same new release, verify their checksums, and replace both binaries.
Keep `~/.agenthub` unchanged.

### npm

```bash
npm install -g @linkerdog/agenthub@latest --include=optional
agenthub --version
```

Upgrade the separately installed `agenthub-acp` archive to the matching release
at the same time.

## Uninstall

### Debian package

```bash
sudo apt remove agenthub
```

Package removal and purge preserve runtime data under `/var/lib/agenthub`.
Back it up and remove it manually only when you intentionally want to delete
all instance state.

### Archive installation

Remove the installed binaries from `$HOME/.local/bin`. Runtime state under
`~/.agenthub` is not removed automatically.

### npm installation

```bash
npm uninstall -g @linkerdog/agenthub
```

This does not remove `~/.agenthub` or a separately installed `agenthub-acp`.

## Install the Web App Shell

On supported browsers, AgentHub can be installed as a standalone web app after
the server is running.

- The frontend registers its service worker automatically.
- The same service worker is used for push notifications and installability.
- AgentHub is not offline-first; a refresh still fetches the current app shell
  and hashed assets from the server.

Use HTTPS for non-localhost deployments if you need browser installability,
passkeys, or push notifications.

## Build From Source

Building from source is a contributor workflow, not the recommended user
installation path. See the repository
[developer setup](https://github.com/hawkingrei/agenthub/blob/main/docs/developer-setup.md)
for the pinned Rust toolchain, web build, Bazel, and test commands.
